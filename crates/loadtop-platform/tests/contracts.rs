//! The port contracts from loadtop-test-support, run against the real adapters. Each test
//! gets its own temporary directory, and signals only a child it spawned itself; nothing
//! touches the real `~/Library` or an existing process.

mod support;

use loadtop_platform::{JsonFileCounterStore, SysinfoProcessTable, SysinfoSignals, SystemClock};
use loadtop_test_support::{
    clock_contract, counter_store_contract, process_table_contract, signals_contract,
};
use support::SpawnedChild;

#[test]
fn json_file_store_meets_the_counter_store_contract() {
    let mut dirs = Vec::new(); // keeps every TempDir alive until the test ends
    counter_store_contract(|| {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileCounterStore::new(dir.path().join("nested").join("counter.json"));
        dirs.push(dir);
        Box::new(store)
    });
}

#[test]
fn system_clock_meets_the_clock_contract() {
    clock_contract(|| Box::new(SystemClock));
}

#[test]
fn sysinfo_process_table_meets_the_process_table_contract() {
    let mut children = Vec::new(); // keeps every child alive until the test ends
    process_table_contract(|| {
        let child = SpawnedChild::sleep();
        let identity = child.identity();
        children.push(child);
        (Box::new(SysinfoProcessTable::new().unwrap()), identity)
    });
}

#[test]
fn sysinfo_signals_meet_the_signals_contract() {
    let mut children = Vec::new();
    signals_contract(|| {
        let child = SpawnedChild::sleep();
        let identity = child.identity();
        children.push(child);
        (Box::new(SysinfoSignals::new().unwrap()), identity)
    });
}
