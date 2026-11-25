//! Listening TCP ports through `lsof`'s machine-readable `-F` output.

use std::collections::HashSet;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use loadtop_core::{ListeningPort, SocketError, SocketTable};

/// How long `lsof` may take before it is killed and the ports are unavailable.
const LSOF_TIMEOUT: Duration = Duration::from_secs(2);

/// How long the streams may take to reach their end once `lsof` has been reaped: its
/// own allowance, so an answer finished just inside [`LSOF_TIMEOUT`] is still read, and
/// the whole call stays under the two together.
const READ_GRACE: Duration = Duration::from_millis(200);

/// How often the wait checks whether `lsof` has exited.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// The socket table, read by running `lsof` once per call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LsofSocketTable {
    lsof: PathBuf,
    uid: u32,
}

impl LsofSocketTable {
    /// A table that runs the `lsof` at `lsof` (the binary passes [`crate::LSOF_PATH`]; a
    /// test passes a script) and asks it about `uid`'s sockets only. `uid` comes from the
    /// process-table adapter, never from user input.
    #[must_use]
    pub const fn new(lsof: PathBuf, uid: u32) -> Self {
        Self { lsof, uid }
    }
}

impl SocketTable for LsofSocketTable {
    fn listening(&self) -> Result<Vec<ListeningPort>, SocketError> {
        let deadline = Instant::now() + LSOF_TIMEOUT;
        let mut child = Command::new(&self.lsof)
            .arg("-a")
            .arg("-nP")
            .arg("-iTCP")
            .arg("-sTCP:LISTEN")
            .arg("-u")
            .arg(self.uid.to_string())
            .arg("-Fpn")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| SocketError::Unavailable)?;
        let streams = drain(child.stdout.take(), child.stderr.take());
        let status = wait_until(&mut child, deadline)?;
        let (stdout, stderr) = streams.collect(READ_GRACE)?;
        match status.code() {
            Some(0) => parse(&stdout),
            Some(1) if stdout.is_empty() && stderr.is_empty() => Ok(vec![]),
            _ => Err(SocketError::Unavailable),
        }
    }
}

/// Wait for `child` until `deadline`; past it, kill and reap it.
fn wait_until(child: &mut Child, deadline: Instant) -> Result<ExitStatus, SocketError> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(POLL_INTERVAL),
            _ => {
                // Already exited is fine: the point is that nothing outlives the call.
                let _ = child.kill();
                let _ = child.wait();
                return Err(SocketError::Unavailable);
            }
        }
    }
}

/// Which stream a reader thread drained.
enum Stream {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
}

/// Both streams, each read to its end on a thread of its own, so a full pipe cannot
/// block the child while the wait polls.
struct Drained {
    receiver: Receiver<Option<Stream>>,
}

fn drain(stdout: Option<ChildStdout>, stderr: Option<ChildStderr>) -> Drained {
    let (sender, receiver) = mpsc::channel();
    let stdout_sender = sender.clone();
    std::thread::spawn(move || {
        let read = stdout.and_then(|mut stream| {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).ok().map(|_| bytes)
        });
        let _ = stdout_sender.send(read.map(Stream::Stdout));
    });
    std::thread::spawn(move || {
        let read = stderr.and_then(|mut stream| {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).ok().map(|_| bytes)
        });
        let _ = sender.send(read.map(Stream::Stderr));
    });
    Drained { receiver }
}

impl Drained {
    /// Both streams' bytes, waiting at most `grace` from now: a process `lsof` left
    /// holding a pipe open must not hang the call, so a reader still blocked then is
    /// left to finish on its own.
    fn collect(self, grace: Duration) -> Result<(Vec<u8>, Vec<u8>), SocketError> {
        let deadline = Instant::now() + grace;
        let mut stdout = None;
        let mut stderr = None;
        while stdout.is_none() || stderr.is_none() {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.receiver.recv_timeout(remaining) {
                Ok(Some(Stream::Stdout(bytes))) => stdout = Some(bytes),
                Ok(Some(Stream::Stderr(bytes))) => stderr = Some(bytes),
                Ok(None) | Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    return Err(SocketError::Unavailable);
                }
            }
        }
        Ok((stdout.unwrap_or_default(), stderr.unwrap_or_default()))
    }
}

/// `lsof -F p n` output as `(pid, port)` pairs, each once. `p<pid>` starts a process,
/// `n<addr>:<port>` names one of its sockets, and every other line is skipped; anything
/// else malformed makes the whole answer unavailable rather than partly right.
fn parse(stdout: &[u8]) -> Result<Vec<ListeningPort>, SocketError> {
    let text = std::str::from_utf8(stdout).map_err(|_| SocketError::Unavailable)?;
    let mut pid = None;
    let mut seen = HashSet::new();
    let mut ports = Vec::new();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix('p') {
            pid = Some(digits(value).ok_or(SocketError::Unavailable)?);
        } else if let Some(address) = line.strip_prefix('n') {
            let pid = pid.ok_or(SocketError::Unavailable)?;
            let port = address
                .rsplit_once(':')
                .and_then(|(_, port)| digits(port))
                .and_then(|port| u16::try_from(port).ok())
                .filter(|port| *port != 0)
                .ok_or(SocketError::Unavailable)?;
            let listening = ListeningPort { pid, port };
            if seen.insert(listening) {
                ports.push(listening);
            }
        }
    }
    Ok(ports)
}

/// A non-empty run of ASCII digits as a number; `u32::from_str` alone would also take a
/// leading `+`.
fn digits(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port(pid: u32, port: u16) -> ListeningPort {
        ListeningPort { pid, port }
    }

    /// Streams whose readers deliver after `delay`, as a reader of a child that only
    /// just exited does; the sender then stays alive, so a stream never sent times out
    /// rather than disconnecting.
    fn drained_after(delay: Duration, stdout: Option<&'static [u8]>) -> Drained {
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            std::thread::sleep(delay);
            if let Some(bytes) = stdout {
                let _ = sender.send(Some(Stream::Stdout(bytes.to_vec())));
            }
            let _ = sender.send(Some(Stream::Stderr(Vec::new())));
            std::thread::sleep(Duration::from_secs(5));
        });
        Drained { receiver }
    }

    #[test]
    fn streams_finishing_after_the_reap_are_still_read_within_the_grace() {
        let drained = drained_after(Duration::from_millis(50), Some(b"p1\nn*:80\n"));
        assert_eq!(
            drained.collect(READ_GRACE),
            Ok((b"p1\nn*:80\n".to_vec(), Vec::new()))
        );
    }

    #[test]
    fn a_stream_that_never_ends_is_unavailable_once_the_grace_runs_out() {
        let drained = drained_after(Duration::ZERO, None);
        let started = Instant::now();
        assert_eq!(drained.collect(READ_GRACE), Err(SocketError::Unavailable));
        let elapsed = started.elapsed();
        assert!(
            elapsed >= READ_GRACE && elapsed < Duration::from_secs(2),
            "waited {elapsed:?}"
        );
    }

    #[test]
    fn several_processes_each_keep_their_ports() {
        let output = b"p91204\nf23\nn127.0.0.1:5173\np81006\nf7\nn*:8000\nf8\nn[::]:8000\n";
        assert_eq!(
            parse(output),
            Ok(vec![port(91204, 5173), port(81006, 8000)])
        );
    }

    #[test]
    fn empty_output_lists_no_port() {
        assert_eq!(parse(b""), Ok(vec![]));
    }

    #[test]
    fn unknown_field_letters_and_blank_lines_are_skipped() {
        assert_eq!(
            parse(b"p100\nf3\nPTCP\n\ntST=LISTEN\nn*:3000\n"),
            Ok(vec![port(100, 3000)])
        );
    }

    #[test]
    fn malformed_output_is_unavailable() {
        for output in [
            &b"p100\nn*:0\n"[..],
            b"p100\nn*:65536\n",
            b"p100\nn*:+80\n",
            b"p100\nn*:http\n",
            b"p100\nn*:\n",
            b"p100\nnlocalhost\n",
            b"n*:80\n",
            b"pabc\nn*:80\n",
            b"p\nn*:80\n",
            b"p-1\nn*:80\n",
            b"p100\nn*:\xff80\n",
        ] {
            assert_eq!(
                parse(output),
                Err(SocketError::Unavailable),
                "{:?}",
                String::from_utf8_lossy(output)
            );
        }
    }

    #[test]
    fn the_highest_port_is_kept() {
        assert_eq!(parse(b"p1\nn*:65535\n"), Ok(vec![port(1, 65535)]));
    }
}
