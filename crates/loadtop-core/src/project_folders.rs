//! Claude Code's project folder names, the [`ProjectFolders`] port that lists them, and
//! what "inside a project" means (requirements §3.3).

use std::path::{Path, PathBuf};

/// Why the project folder names could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProjectFoldersError {
    /// The projects directory exists but could not be listed.
    #[error("the project folders could not be read")]
    Unreadable,
}

/// The folder names Claude Code keeps one per project it has run in. A port: the
/// platform adapter lists the real directory, a fake returns hand-built names.
pub trait ProjectFolders: Send + Sync {
    /// Every folder name, in no particular order; a missing directory is `Ok(vec![])`,
    /// since Claude Code has not run anywhere yet.
    ///
    /// # Errors
    /// [`ProjectFoldersError::Unreadable`] when the directory cannot be listed.
    fn names(&self) -> Result<Vec<String>, ProjectFoldersError>;
}

/// A path as a project folder name: each `char` that is not ASCII alphanumeric becomes
/// one `-` (`/Users/ann/code/acme-web` → `-Users-ann-code-acme-web`). A path that is not
/// valid UTF-8 is read lossily, each invalid sequence becoming one `-`.
///
/// If this differs from Claude Code's own encoding, a folder-name lookup only misses:
/// a session's project never comes from a folder name, so nothing is mislabeled or
/// signaled because of it.
#[must_use]
pub fn encode_project_path(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

/// Whether `cwd` is inside a project: `cwd` or one of its ancestors, never `home`, an
/// ancestor of `home` (such as `/Users`), or `/`, encodes ([`encode_project_path`]) to exactly the encoding of a live
/// session's cwd or to a project folder name. Each ancestor matches exactly, so
/// `~/foo-bar` is not inside `~/foo`.
#[must_use]
pub fn is_inside_project(
    cwd: &Path,
    home: &Path,
    live_session_cwds: &[PathBuf],
    folder_names: &[String],
) -> bool {
    let live: Vec<String> = live_session_cwds
        .iter()
        .map(|path| encode_project_path(path))
        .collect();
    cwd.ancestors()
        .filter(|ancestor| {
            !home.starts_with(ancestor)
                && ancestor.parent().is_some()
                && !ancestor.as_os_str().is_empty()
        })
        .map(encode_project_path)
        .any(|encoded| live.contains(&encoded) || folder_names.contains(&encoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    mod encode {
        use super::*;

        #[test]
        fn every_non_alphanumeric_char_becomes_a_dash() {
            assert_eq!(
                encode_project_path(Path::new("/Users/ann/code/acme_web.v2")),
                "-Users-ann-code-acme-web-v2"
            );
        }

        #[test]
        fn a_non_ascii_char_becomes_one_dash() {
            assert_eq!(
                encode_project_path(Path::new("/Users/ann/code/r\u{e9}sum\u{e9}")),
                "-Users-ann-code-r-sum-"
            );
        }
    }

    mod inside_project {
        use super::*;

        const HOME: &str = "/Users/ann";

        fn inside(cwd: &str, live: &[&str], folders: &[&str]) -> bool {
            let live: Vec<PathBuf> = live.iter().map(PathBuf::from).collect();
            let folders: Vec<String> = folders.iter().map(|name| (*name).to_owned()).collect();
            is_inside_project(Path::new(cwd), Path::new(HOME), &live, &folders)
        }

        #[test]
        fn a_sibling_with_a_longer_name_is_not_inside_the_project() {
            assert!(!inside("/Users/ann/foo-bar/src", &["/Users/ann/foo"], &[]));
        }

        #[test]
        fn an_ancestor_matching_a_project_folder_is_inside_the_project() {
            assert!(inside(
                "/Users/ann/code/acme-api/scripts",
                &[],
                &["-Users-ann-code-acme-api"]
            ));
        }

        #[test]
        fn an_ancestor_matching_a_live_session_cwd_is_inside_the_project() {
            assert!(inside(
                "/Users/ann/code/acme-web/node_modules/.bin",
                &["/Users/ann/code/acme-web"],
                &[]
            ));
        }

        #[test]
        fn the_cwd_itself_matching_is_inside_the_project() {
            assert!(inside(
                "/Users/ann/code/acme-api",
                &[],
                &["-Users-ann-code-acme-api"]
            ));
        }

        #[test]
        fn the_home_directory_is_never_a_project() {
            assert!(!inside("/Users/ann", &["/Users/ann"], &["-Users-ann"]));
        }

        #[test]
        fn a_folder_under_home_is_not_inside_a_project_at_home() {
            assert!(!inside(
                "/Users/ann/Downloads",
                &["/Users/ann"],
                &["-Users-ann"]
            ));
        }

        #[test]
        fn an_ancestor_of_home_is_never_a_project() {
            assert!(!inside("/Users/ann/Downloads", &["/Users"], &["-Users"]));
        }

        #[test]
        fn a_project_outside_home_still_matches() {
            assert!(inside("/opt/acme/src", &[], &["-opt-acme"]));
        }

        #[test]
        fn the_root_is_never_a_project() {
            assert!(!inside("/", &["/"], &["-"]));
        }

        #[test]
        fn nothing_matches_without_sessions_or_folders() {
            assert!(!inside("/Users/ann/code/acme-api", &[], &[]));
        }
    }
}
