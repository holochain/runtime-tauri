use hdk::prelude::*;
use test_integrity::{EntryTypes, TestEntry, UnitEntryTypes};

/// Commit a `TestEntry` with the given content.
#[hdk_extern]
pub fn create_test_entry(content: String) -> ExternResult<ActionHash> {
    create_entry(EntryTypes::TestEntry(TestEntry { content }))
}

/// Every `TestEntry` on this agent's chain; empty on a fresh cell.
#[hdk_extern]
pub fn get_test_entries(_: ()) -> ExternResult<Vec<Record>> {
    let filter = ChainQueryFilter::new()
        .entry_type(UnitEntryTypes::TestEntry.try_into()?)
        .include_entries(true);
    query(filter)
}

/// Emit an app signal for every commit, so tests can observe signal delivery.
#[hdk_extern]
pub fn post_commit(committed: Vec<SignedActionHashed>) -> ExternResult<()> {
    emit_signal(committed.len())
}
