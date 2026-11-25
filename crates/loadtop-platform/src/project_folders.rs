//! Claude Code's project folder names, listed from `~/.claude/projects/`. Nothing inside
//! a folder is opened: they hold transcripts.

use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::PathBuf;

use loadtop_core::{ProjectFolders, ProjectFoldersError};

/// Project folder names, listed from one directory on every call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFoldersDir {
    dir: PathBuf,
}

impl ProjectFoldersDir {
    /// Folder names listed from `dir` (the binary passes [`crate::claude_projects_dir`]).
    #[must_use]
    pub const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl ProjectFolders for ProjectFoldersDir {
    fn names(&self) -> Result<Vec<String>, ProjectFoldersError> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(vec![]),
            Err(_) => return Err(ProjectFoldersError::Unreadable),
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| ProjectFoldersError::Unreadable)?;
            if let Some(name) = folder_name(entry.file_name()) {
                names.push(name);
            }
        }
        Ok(names)
    }
}

/// An entry's name as a folder name; a name that is not UTF-8 can never equal an encoded
/// path, so it is skipped.
fn folder_name(name: OsString) -> Option<String> {
    name.into_string().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_utf8_name_is_kept() {
        assert_eq!(
            folder_name(OsString::from("-Users-ann-code-acme-web")),
            Some("-Users-ann-code-acme-web".to_owned())
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_non_utf8_name_is_skipped() {
        use std::os::unix::ffi::OsStringExt;
        assert_eq!(
            folder_name(OsString::from_vec(b"-Users-ann-\xff".to_vec())),
            None
        );
    }
}
