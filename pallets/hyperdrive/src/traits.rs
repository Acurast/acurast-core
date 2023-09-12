use frame_support::weights::Weight;

/// Weight functions needed for pallet_acurast_hyperdrive.
pub trait WeightInfo {
    fn update_state_transmitters(l: u32) -> Weight;
    fn submit_state_merkle_root() -> Weight;
    fn submit_message() -> Weight;
    fn update_target_chain_owner() -> Weight;
}
