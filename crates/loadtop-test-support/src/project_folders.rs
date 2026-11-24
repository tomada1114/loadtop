use loadtop_core::{ProjectFolders, ProjectFoldersError};

/// Project folders that answer with a fixed list of names, or fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeProjectFolders {
    answer: Result<Vec<String>, ProjectFoldersError>,
}

impl FakeProjectFolders {
    /// Folders that read back as `names`; an empty list is a missing directory.
    #[must_use]
    pub const fn new(names: Vec<String>) -> Self {
        Self { answer: Ok(names) }
    }

    /// Folders whose every read fails with [`ProjectFoldersError::Unreadable`].
    #[must_use]
    pub const fn unreadable() -> Self {
        Self {
            answer: Err(ProjectFoldersError::Unreadable),
        }
    }
}

impl ProjectFolders for FakeProjectFolders {
    fn names(&self) -> Result<Vec<String>, ProjectFoldersError> {
        self.answer.clone()
    }
}

fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

/// The behaviour every [`ProjectFolders`] must have. `make(None)` returns folders over a
/// missing directory; `make(Some(names))` returns folders over a directory holding
/// exactly those folder names.
///
/// # Panics
/// When the folders break the contract; that is how the calling test fails.
pub fn project_folders_contract(mut make: impl FnMut(Option<&[&str]>) -> Box<dyn ProjectFolders>) {
    assert_eq!(
        make(None).names(),
        Ok(vec![]),
        "a missing directory holds no folders"
    );
    assert_eq!(
        make(Some(&[])).names(),
        Ok(vec![]),
        "an empty directory holds no folders"
    );

    let written = ["-Users-ann-code-acme-web", "-Users-ann-code-acme-api"];
    let expected = sorted(written.iter().map(|name| (*name).to_owned()).collect());
    assert_eq!(
        make(Some(&written)).names().map(sorted),
        Ok(expected),
        "the names read back as written, in any order"
    );
}
