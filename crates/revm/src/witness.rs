use alloc::vec::Vec;
use alloy_primitives::{keccak256, Bytes, B256};
use reth_trie::{HashedPostState, HashedStorage};
use revm::database::State;

/// Tracks state changes during execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionWitnessRecord {
    /// Records all state changes
    pub hashed_state: HashedPostState,
    /// Map of all contract codes (created / accessed) to their preimages that were required during
    /// the execution of the block, including during state root recomputation.
    ///
    /// `keccak(bytecodes) => bytecodes`
    pub codes: Vec<Bytes>,
    /// Map of all hashed account and storage keys (addresses and slots) to their preimages
    /// (unhashed account addresses and storage slots, respectively) that were required during
    /// the execution of the block.
    ///
    /// `keccak(address|slot) => address|slot`
    pub keys: Vec<Bytes>,
    /// The lowest block number referenced by any BLOCKHASH opcode call during transaction
    /// execution.
    ///
    /// This helps determine which ancestor block headers must be included in the
    /// `ExecutionWitness`.
    ///
    /// `None` - when the BLOCKHASH opcode was not called during execution
    pub lowest_block_number: Option<u64>,
}

impl ExecutionWitnessRecord {
    /// Records the state after execution.
    pub fn record_executed_state<DB>(&mut self, statedb: &State<DB>) {
        self.codes = statedb
            .cache
            .contracts
            .values()
            .map(|code| code.original_bytes())
            .chain(
                // cache state does not have all the contracts, especially when
                // a contract is created within the block
                // the contract only exists in bundle state, therefore we need
                // to include them as well
                statedb.bundle_state.contracts.values().map(|code| code.original_bytes()),
            )
            .collect();

        for (address, account) in &statedb.cache.accounts {
            let hashed_address = keccak256(address);
            self.hashed_state
                .accounts
                .insert(hashed_address, account.account.as_ref().map(|a| (&a.info).into()));

            let storage = self
                .hashed_state
                .storages
                .entry(hashed_address)
                .or_insert_with(|| HashedStorage::new(account.status.was_destroyed()));

            if let Some(account) = &account.account {
                self.keys.push(address.to_vec().into());

                for (slot, value) in &account.storage {
                    let slot = B256::from(*slot);
                    let hashed_slot = keccak256(slot);
                    storage.storage.insert(hashed_slot, *value);

                    self.keys.push(slot.into());
                }
            }
        }
        // BTreeMap keys are ordered, so the first key is the smallest
        self.lowest_block_number = statedb.block_hashes.keys().next().copied()
    }

    /// Creates the record from the state after execution.
    pub fn from_executed_state<DB>(state: &State<DB>) -> Self {
        let mut record = Self::default();
        record.record_executed_state(state);
        record
    }
}
