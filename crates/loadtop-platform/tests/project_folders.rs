//! What the project-folder lister does beyond the port's contract, each case in a
//! temporary directory standing in for `~/.claude/projects/`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use loadtop_core::{ProjectFolders, ProjectFoldersError};
use loadtop_platform::ProjectFoldersDir;

fn names_in(dir: &Path) -> Result<Vec<String>, ProjectFoldersError> {
    ProjectFoldersDir::new(dir.to_path_buf()).names()
}

#[test]
fn folder_names_are_read_without_opening_them() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("-Users-ann-code-acme-web");
    std::fs::create_dir(&folder).unwrap();
    let transcript = folder.join("3f9c2a10-0000-4000-8000-000000000001.jsonl");
    std::fs::write(&transcript, "{}\n").unwrap();
    std::fs::set_permissions(&transcript, std::fs::Permissions::from_mode(0o000)).unwrap();

    assert_eq!(
        names_in(dir.path()),
        Ok(vec!["-Users-ann-code-acme-web".to_owned()])
    );
}

#[test]
fn a_file_or_a_symlink_beside_the_folders_is_not_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("-Users-ann-code-acme-web");
    std::fs::create_dir(&folder).unwrap();
    std::fs::write(dir.path().join(".DS_Store"), "").unwrap();
    std::os::unix::fs::symlink(&folder, dir.path().join("-Users-ann-code-acme-api")).unwrap();
    assert_eq!(
        names_in(dir.path()),
        Ok(vec!["-Users-ann-code-acme-web".to_owned()])
    );
}

#[test]
fn a_missing_directory_holds_no_folders() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(names_in(&dir.path().join("projects")), Ok(vec![]));
}

#[test]
fn an_unlistable_directory_is_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("-Users-ann-code-acme-web")).unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o000)).unwrap();
    let names = names_in(dir.path());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(names, Err(ProjectFoldersError::Unreadable));
}

// APFS refuses a name that is not UTF-8, so only Linux can create one.
#[cfg(target_os = "linux")]
#[test]
fn a_non_utf8_folder_name_is_skipped() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(OsStr::from_bytes(b"-Users-ann-\xff"))).unwrap();
    std::fs::create_dir(dir.path().join("-Users-ann-code-acme-api")).unwrap();
    assert_eq!(
        names_in(dir.path()),
        Ok(vec!["-Users-ann-code-acme-api".to_owned()])
    );
}
