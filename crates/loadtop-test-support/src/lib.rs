//! Fakes for every `loadtop-core` port, and one contract function per port.
//!
//! A contract function holds the behaviour every implementation of a port must have.
//! `loadtop-core`'s integration tests run it against the fake here; `loadtop-platform`'s
//! tests run the same function against the real adapter — so the fake cannot drift
//! from the real thing without a test failing.
//!
//! This crate is a `[dev-dependencies]` entry only; a harness check fails if a normal
//! dependency edge points at it. Its functions are library code, not tests, so they
//! compare `Result`s with `assert_eq!` instead of unwrapping.

mod clock;
mod counter_store;
mod process_table;
mod project_folders;
mod session_records;
mod signals;
mod socket_table;
mod text_generator;

pub use clock::{FixedClock, clock_contract};
pub use counter_store::{FailingCounterStore, InMemoryCounterStore, counter_store_contract};
pub use process_table::{FakeProcessTable, process_table_contract, process_with, sample_of};
pub use project_folders::{FakeProjectFolders, project_folders_contract};
pub use session_records::{FakeSessionRecords, session_records_contract};
pub use signals::{FakeSignals, signals_contract};
pub use socket_table::{FakeSocketTable, socket_table_contract};
pub use text_generator::{StubTextGenerator, text_generator_contract};
