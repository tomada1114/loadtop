//! The ordered rows the screen selects in and the view draws: values and variants only,
//! never text (ux-flows S1; the visual grammar's Tree and Folded rows).

use std::slice;

use crate::classify::{Classification, Group, GroupKind, Node, Totals};
use crate::list::fold::{FoldState, Folds, RowId};
use crate::process_table::{ProcessIdentity, ProcessInfo, RunState};
use crate::tuning::FoldTuning;

/// One row of the list, borrowing the classification it was built from.
#[derive(Debug, Clone, PartialEq)]
pub enum Row<'a> {
    /// A session or leftover group, with its fold state.
    Group {
        /// The group, its whole-tree totals included.
        group: &'a Group,
        /// How much of its tree is shown.
        fold: FoldState,
    },
    /// A process in its group's tree.
    Process {
        /// The process; its children are drawn as the rows that follow.
        node: &'a Node,
        /// The root identity of its group.
        group: ProcessIdentity,
        /// 1 for a subtree's top, one more per level below.
        depth: usize,
        /// For each level from 1 to `depth`, whether the row's ancestor at that level
        /// (the row itself last) is the last sibling there, a `+N more` row counting
        /// as the last at level 1: what the view needs to draw `├─ └─ │`.
        last: Vec<bool>,
    },
    /// The processes of a group's folded subtrees that are not lifted.
    More {
        /// The root identity of its group.
        group: ProcessIdentity,
        /// How many processes it stands for.
        count: usize,
        /// Their summed latest CPU % and resident memory.
        totals: Totals,
    },
    /// A paused process lifted out of a collapsed group or a `+N more` fold, below the
    /// group's other rows, so a paused process is never folded away.
    Lifted {
        /// The paused process.
        node: &'a Node,
        /// The root identity of its group.
        group: ProcessIdentity,
    },
}

impl Row<'_> {
    /// The row's identity, for the selection to follow.
    #[must_use]
    pub fn id(&self) -> RowId {
        match self {
            Row::Group { group, .. } => RowId::Group(group.root.process.identity),
            Row::Process { node, .. } | Row::Lifted { node, .. } => {
                RowId::Process(node.process.identity)
            }
            Row::More { group, .. } => RowId::More(*group),
        }
    }
}

/// The rows of `classification` in its order, each group folded as `folds` says.
///
/// A session's group row stands for its `claude` process, so its subtrees are the
/// root's children; a leftover's top process is a target of its own, so its one
/// subtree is the whole tree, top included. A folded group shows its first
/// `tuning.subtrees_per_group` subtrees whole and one `+N more` row for the rest; an
/// expanded one shows every subtree; a collapsed one only its group row. Every paused
/// process a fold hides is lifted out, after the group's other rows, and left out of
/// the `+N more` row, which is not emitted when it would stand for nothing.
#[must_use]
pub fn rows<'a>(
    classification: &'a Classification,
    folds: &Folds,
    tuning: FoldTuning,
) -> Vec<Row<'a>> {
    let mut out = Vec::new();
    for group in &classification.groups {
        let id = group.root.process.identity;
        let fold = folds.state(id);
        out.push(Row::Group { group, fold });
        let subtrees: &[Node] = match group.kind {
            GroupKind::Session(_) => &group.root.children,
            GroupKind::Leftover { .. } => slice::from_ref(&group.root),
        };
        let shown_count = match fold {
            FoldState::Folded => tuning.subtrees_per_group.min(subtrees.len()),
            FoldState::Expanded => subtrees.len(),
            FoldState::Collapsed => 0,
        };
        let (shown, folded) = subtrees.split_at(shown_count);

        let mut lifted = Vec::new();
        let mut count = 0;
        let mut totals = Totals {
            cpu_percent: None,
            rss_bytes: 0,
        };
        for node in folded.iter().flat_map(preorder) {
            match node.process.state {
                RunState::Stopped => lifted.push(node),
                RunState::Running => {
                    count += 1;
                    totals = plus(totals, &node.process);
                }
            }
        }
        let more = match fold {
            FoldState::Folded => count > 0,
            FoldState::Expanded | FoldState::Collapsed => false,
        };

        for (at, node) in shown.iter().enumerate() {
            let last = at + 1 == shown.len() && !more;
            push_tree(&mut out, node, id, &[last]);
        }
        if more {
            out.push(Row::More {
                group: id,
                count,
                totals,
            });
        }
        out.extend(
            lifted
                .into_iter()
                .map(|node| Row::Lifted { node, group: id }),
        );
    }
    out
}

fn push_tree<'a>(out: &mut Vec<Row<'a>>, node: &'a Node, group: ProcessIdentity, last: &[bool]) {
    let children = node.children.len();
    out.push(Row::Process {
        node,
        group,
        depth: last.len(),
        last: last.to_vec(),
    });
    for (at, child) in node.children.iter().enumerate() {
        let mut below = last.to_vec();
        below.push(at + 1 == children);
        push_tree(out, child, group, &below);
    }
}

/// `node` and everything under it, parents before children, in the tree's order.
fn preorder(node: &Node) -> Vec<&Node> {
    let mut order = Vec::new();
    let mut stack = vec![node];
    while let Some(next) = stack.pop() {
        order.push(next);
        stack.extend(next.children.iter().rev());
    }
    order
}

fn plus(totals: Totals, process: &ProcessInfo) -> Totals {
    let cpu_percent = match (totals.cpu_percent, process.cpu_percent) {
        (Some(a), Some(b)) => Some(a + b),
        (Some(a), None) | (None, Some(a)) => Some(a),
        (None, None) => None,
    };
    Totals {
        cpu_percent,
        rss_bytes: totals.rss_bytes.saturating_add(process.rss_bytes),
    }
}
