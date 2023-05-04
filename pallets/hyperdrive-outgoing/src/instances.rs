pub mod tezos {

    /// The tezos target chain instance.
    pub type TargetChainTezos = crate::Instance1;

    pub const INDEXING_PREFIX: &'static [u8] = b"mmr-tez-";
    pub const TEMP_INDEXING_PREFIX: &'static [u8] = b"mmr-tez-temp-";

    #[cfg(feature = "std")]
    mod rpc {
        use crate::rpc::RpcInstance;

        impl RpcInstance for super::TargetChainTezos {
            const SNAPSHOT_ROOTS: &'static str = "hyperdrive_outgoing_tezos_snapshotRoots";
            const SNAPSHOT_ROOT: &'static str = "hyperdrive_outgoing_tezos_snapshotRoot";
            const GENERATE_PROOF: &'static str = "hyperdrive_outgoing_tezos_generateProof";
        }
    }
}
