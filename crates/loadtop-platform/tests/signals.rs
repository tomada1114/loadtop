//! What the sysinfo signal sender does beyond the port's contract. Every signal goes to a
//! child the test spawned.

mod support;

use std::time::{Duration, Instant};

use loadtop_core::{ProcessIdentity, Signal, SignalOutcome, Signals};
use loadtop_platform::SysinfoSignals;
use support::SpawnedChild;

fn signals() -> SysinfoSignals {
    match SysinfoSignals::new() {
        Ok(signals) => signals,
        Err(error) => panic!("the signal sender could not be opened: {error:?}"),
    }
}

#[test]
fn a_reused_pid_is_gone_and_is_not_signaled() {
    let mut child = SpawnedChild::sleep();
    let identity = child.identity();
    let other_start = ProcessIdentity {
        start_time: identity.start_time + 1,
        ..identity
    };

    assert_eq!(
        signals().send(other_start, Signal::Kill),
        Ok(SignalOutcome::Gone)
    );
    // Give a stray SIGKILL time to land before checking that none was sent.
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        matches!(child.child.try_wait(), Ok(None)),
        "the spawned sleep is still running"
    );
}

#[test]
fn a_zombie_is_gone_and_is_not_signaled() {
    let mut child = SpawnedChild::sleep();
    let identity = child.identity();
    let signals = signals();
    assert_eq!(
        signals.send(identity, Signal::Cont),
        Ok(SignalOutcome::Sent)
    );

    // Killed through the standard library and deliberately not reaped: a zombie.
    if let Err(error) = child.child.kill() {
        panic!("the test child could not be killed: {error}");
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut outcome = signals.send(identity, Signal::Stop);
    while outcome != Ok(SignalOutcome::Gone) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
        outcome = signals.send(identity, Signal::Stop);
    }
    assert_eq!(
        outcome,
        Ok(SignalOutcome::Gone),
        "an unreaped, exited child is gone"
    );
}

#[test]
fn a_reaped_child_is_gone() {
    let mut child = SpawnedChild::sleep();
    let identity = child.identity();
    if let Err(error) = child.child.kill() {
        panic!("the test child could not be killed: {error}");
    }
    if let Err(error) = child.child.wait() {
        panic!("the test child could not be reaped: {error}");
    }
    assert_eq!(
        signals().send(identity, Signal::Term),
        Ok(SignalOutcome::Gone)
    );
}
