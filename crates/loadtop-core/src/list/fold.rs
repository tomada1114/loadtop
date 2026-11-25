//! Which groups the user expanded or collapsed, kept across refreshes by the group's
//! root identity (requirements §3.2 Collapse).

use std::collections::HashMap;

use crate::classify::Classification;
use crate::process_table::ProcessIdentity;

/// How much of a group's tree the list shows. Only group rows hold one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum FoldState {
    /// The launch state: the heaviest subtrees whole, the rest in one `+N more` row.
    #[default]
    Folded,
    /// Every subtree.
    Expanded,
    /// The group row alone.
    Collapsed,
}

/// Which row of the list something is, for the selection to follow across refreshes.
/// A process keeps its identity whether it is shown in its tree or lifted out of a
/// fold, so a selection follows it either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RowId {
    /// A group row, by its root's identity (a leftover's top process).
    Group(ProcessIdentity),
    /// A process row, in its tree or lifted.
    Process(ProcessIdentity),
    /// The `+N more` row of the group with this root identity.
    More(ProcessIdentity),
}

/// Every group's fold state, by the group's root identity. A group not held is
/// [`FoldState::Folded`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Folds {
    states: HashMap<ProcessIdentity, FoldState>,
}

impl Folds {
    /// The fold state of the group whose root is `group`.
    #[must_use]
    pub fn state(&self, group: ProcessIdentity) -> FoldState {
        self.states.get(&group).copied().unwrap_or_default()
    }

    /// Expands the group of `row`: a group row or its `+N more` row. A process row's
    /// expand is not a fold change, so it returns the state unchanged.
    #[must_use]
    pub fn expand(self, row: RowId) -> Self {
        self.set(row, FoldState::Expanded)
    }

    /// Collapses the group of `row`: a group row or its `+N more` row. A process row's
    /// collapse is not a fold change, so it returns the state unchanged.
    #[must_use]
    pub fn collapse(self, row: RowId) -> Self {
        self.set(row, FoldState::Collapsed)
    }

    /// Keeps only the groups `classification` lists, so a group that disappeared comes
    /// back, if ever, in its launch state.
    #[must_use]
    pub fn retain_present(mut self, classification: &Classification) -> Self {
        let present: std::collections::HashSet<ProcessIdentity> = classification
            .groups
            .iter()
            .map(|group| group.root.process.identity)
            .collect();
        self.states.retain(|group, _| present.contains(group));
        self
    }

    fn set(mut self, row: RowId, state: FoldState) -> Self {
        match row {
            RowId::Group(group) | RowId::More(group) => {
                self.states.insert(group, state);
            }
            RowId::Process(_) => {}
        }
        self
    }
}
