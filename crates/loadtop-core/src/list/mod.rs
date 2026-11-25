//! The list the screen shows: the order over the last few samples, each group's fold
//! state, and the rows that come of them (requirements §3.2 Collapse, Order stability,
//! Default order). `--once` and `--json` never use it: they print one sample, unfolded.

pub mod fold;
pub mod history;
pub mod rows;

pub use fold::{FoldState, Folds, RowId};
pub use history::{History, Reading, order_by_window};
pub use rows::{Row, rows};
