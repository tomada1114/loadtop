//! The app's domain logic.
//!
//! Core holds the rules and the state; it never touches the operating system. Time,
//! storage, and anything else outside the process reach it through *ports* — traits
//! declared here and implemented by `loadtop-platform` (the real thing) or
//! `loadtop-test-support` (fakes). See `AGENTS.md` › Architecture.

// Every `match` on a core enum names each variant, so adding a variant is a compile
// error at every place that must decide what it means (the `writing-rust` skill).
#![deny(clippy::wildcard_enum_match_arm)]

pub mod classify;
pub mod counter;
pub mod process_table;
pub mod project_folders;
pub mod report;
pub mod sample;
pub mod session_records;
pub mod signals;
pub mod socket_table;
pub mod time;
pub mod tuning;

pub use classify::{
    Classification, Group, GroupKind, GuardReason, LeftoverTier, Node, PortsView, SessionFacts,
    SessionKind, SortKey, Totals, classify, home_relative,
};
pub use counter::{
    Counter, CounterError, CounterScreen, CounterService, CounterView, ScreenAction, ScreenKey,
    StorageError, StorageErrorKind, StoredCounter, Tuning, TuningError, store::CounterStore,
};
pub use process_table::{
    ChildKind, EnvRead, HELPER_ARGS, MachineTotals, Markers, ProcessIdentity, ProcessInfo,
    ProcessSample, ProcessTable, ProcessTableError, RunState, Terminal, child_kind,
    is_helper_invocation,
};
pub use project_folders::{
    ProjectFolders, ProjectFoldersError, encode_project_path, is_inside_project,
};
pub use report::{
    ClaudeReport, JsonReport, LeftoverReport, MachineReport, NestedSessionReport,
    ProcessKindReport, ProcessReport, SCHEMA_VERSION, SessionKindReport, SessionReport,
    StateReport, TierReport,
};
pub use sample::{Sample, SamplePorts};
pub use session_records::{SessionRecord, SessionRecords, SessionRecordsError, record_is_valid};
pub use signals::{Signal, SignalError, SignalOutcome, Signals};
pub use socket_table::{ListeningPort, SocketError, SocketTable};
pub use time::{Clock, UnixMillis};
pub use tuning::{
    AppTuning, AppTuningError, FoldTuning, HeatTuning, OrderTuning, SamplingTuning, StopTuning,
    TrendTuning,
};
