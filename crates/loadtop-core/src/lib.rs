//! The app's domain logic.
//!
//! Core holds the rules and the state; it never touches the operating system. Time,
//! storage, and anything else outside the process reach it through *ports* — traits
//! declared here and implemented by `loadtop-platform` (the real thing) or
//! `loadtop-test-support` (fakes). See `AGENTS.md` › Architecture.

// Every `match` on a core enum names each variant, so adding a variant is a compile
// error at every place that must decide what it means (the `writing-rust` skill).
#![deny(clippy::wildcard_enum_match_arm)]

pub mod counter;
pub mod time;

pub use counter::{
    Counter, CounterError, CounterScreen, CounterService, CounterView, ScreenAction, ScreenKey,
    StorageError, StorageErrorKind, StoredCounter, Tuning, TuningError, store::CounterStore,
};
pub use time::{Clock, UnixMillis};
