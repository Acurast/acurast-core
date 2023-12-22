#![cfg_attr(not(feature = "std"), no_std, no_main)]

use ink::env::call::Selector;

pub use validator::Error;

// Method selectors

pub const VERIFY_PROOF_SELECTOR: Selector = Selector::new(ink::selector_bytes!("verify_proof"));

// Method types

pub type VerifyProofReturn = Result<bool, validator::Error>;

#[ink::contract]
pub mod validator {
    use ink::env::hash;
    use ink::prelude::vec;
    use ink::prelude::{format, string::String, vec::Vec};
    use ink::storage::{traits::Packed, Mapping};
    use scale::{Decode, Encode, EncodeLike};

    use ckb_merkle_mountain_range::{Error as MMRError, Merge, MerkleProof as MMRMerkleProof};

    struct MergeKeccak;

    impl Merge for MergeKeccak {
        type Item = [u8; 32];
        fn merge(lhs: &Self::Item, rhs: &Self::Item) -> Result<Self::Item, MMRError> {
            let mut concat = vec![];
            concat.extend(lhs);
            concat.extend(rhs);

            let mut output = <hash::Keccak256 as hash::HashOutput>::Type::default();
            ink::env::hash_bytes::<hash::Keccak256>(&concat, &mut output);

            Ok(output.try_into().expect("INVALID_HASH_LENGTH"))
        }
    }

    #[derive(Decode, Encode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct LeafProof {
        pub leaf_index: u64,
        pub data: Vec<u8>,
    }

    #[derive(Decode, Encode, Debug)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub struct MerkleProof<T: Decode + Packed + EncodeLike> {
        pub mmr_size: u64,
        pub proof: Vec<T>,
        pub leaves: Vec<LeafProof>,
    }

    const MAX_VALIDATORS: usize = 50;

    /// Errors returned by the contract's methods.
    #[derive(Debug, PartialEq, Eq, Encode, Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        ProofInvalid(String),
        SnapshotUnknown,
    }

    /// A custom type that we can use in our contract storage
    #[ink::storage_item]
    #[derive(Debug)]
    pub struct Config {
        /// Multi-sig address allowed to manage the contract
        governance_address: AccountId,
        /// Minimum expected endorsements for a given state root to be considered valid
        minimum_endorsements: u16,
        /// Validators
        validators: Vec<AccountId>,
    }

    /// Defines the storage of your contract.
    /// Add new fields to the below struct in order
    /// to add new static storage fields to your contract.
    #[ink(storage)]
    pub struct Validator {
        config: Config,
        current_snapshot: u128,
        root: Mapping<u128, [u8; 32]>,
        snapshot_submissions: Mapping<AccountId, [u8; 32]>,
        snapshot_submissions_accounts: Vec<AccountId>,
    }

    impl Validator {
        #[ink(constructor)]
        pub fn new(
            admin: AccountId,
            minimum_endorsements: u16,
            mut validators: Vec<AccountId>,
        ) -> Self {
            assert!(validators.len() <= MAX_VALIDATORS, "TOO_MANY_VALIDATORS");
            assert!(minimum_endorsements > 0, "NON_ZERO_ENDORSEMENTS");

            let mut contract = Self::default();
            validators.sort_unstable();
            validators.dedup();

            contract.config.validators = validators;
            contract.config.governance_address = admin;
            contract.config.minimum_endorsements = minimum_endorsements;
            contract
        }

        #[ink(constructor)]
        pub fn default() -> Self {
            Self {
                config: Config {
                    governance_address: AccountId::from([0x0; 32]),
                    minimum_endorsements: 0,
                    validators: vec![],
                },
                current_snapshot: 1,
                root: Default::default(),
                snapshot_submissions: Default::default(),
                snapshot_submissions_accounts: Default::default(),
            }
        }

        fn fail_if_not_validator(&self, account: &AccountId) {
            assert!(self.config.validators.contains(account), "NOT_ALLOWED");
        }

        fn validate_block_state_root(&self) -> Option<[u8; 32]> {
            let mut endorsements_per_root: Mapping<[u8; 32], u128> = Default::default();
            let mut candidate_roots = vec![];

            for account in self.snapshot_submissions_accounts.iter() {
                if let Some(hash) = self.snapshot_submissions.get(account) {
                    let submissions = endorsements_per_root.get(hash).unwrap_or(0);
                    endorsements_per_root.insert(hash, &(submissions + 1));

                    if !candidate_roots.contains(&hash) {
                        candidate_roots.push(hash);
                    }
                }
            }

            let mut selected_candidate: [u8; 32] = [0; 32];
            let mut selected_candidade_submissions = 0;
            for candidate in candidate_roots {
                let submissions = endorsements_per_root.get(candidate).unwrap_or(0);
                if u128::from(selected_candidade_submissions) < submissions {
                    selected_candidate = candidate;
                    selected_candidade_submissions = submissions;
                }
            }

            if selected_candidade_submissions < self.config.minimum_endorsements.into() {
                return None;
            }

            Some(selected_candidate)
        }

        #[ink(message)]
        pub fn submit_root(&mut self, snapshot: u128, root: [u8; 32]) {
            let caller = Self::env().caller();

            // Check if sender is a validator
            Self::fail_if_not_validator(self, &caller);

            // Make sure the snapshots are submitted sequencially
            assert!(self.current_snapshot == snapshot, "INVALID_SNAPSHOT");

            if !self.snapshot_submissions.contains(caller) {
                self.snapshot_submissions_accounts.push(caller);
            }

            // Store the root per validator
            self.snapshot_submissions.insert(caller, &root);

            // Finalize snapshot if consensus has been reached
            let can_finalize_snapshot = Self::validate_block_state_root(self);

            if can_finalize_snapshot.is_some() {
                self.root.insert(self.current_snapshot, &root);
                self.current_snapshot += 1;
                self.snapshot_submissions = Default::default();
            }
        }

        //
        // Views
        //

        #[ink(message)]
        pub fn verify_proof(
            &self,
            snapshot: u128,
            proof: MerkleProof<[u8; 32]>,
        ) -> crate::VerifyProofReturn {
            // Get snapshot root
            let snaptshot_root = self.root.get(snapshot).ok_or(Error::SnapshotUnknown)?;

            // Prepare proof instance
            let mmr_proof =
                MMRMerkleProof::<[u8; 32], MergeKeccak>::new(proof.mmr_size, proof.proof);

            // Derive root from proof and leaves
            let hashed_leaves: Vec<(u64, [u8; 32])> = proof
                .leaves
                .iter()
                .map(|item| {
                    let mut hash = <hash::Keccak256 as hash::HashOutput>::Type::default();
                    ink::env::hash_bytes::<hash::Keccak256>(&item.data, &mut hash);

                    match <[u8; 32]>::try_from(hash) {
                        Ok(h) => Ok((item.leaf_index, h)),
                        Err(err) => Err(Error::ProofInvalid(format!("{:?}", err))),
                    }
                })
                .collect::<Result<Vec<(u64, [u8; 32])>, Error>>()?;

            match mmr_proof.calculate_root(hashed_leaves) {
                Err(err) => Err(Error::ProofInvalid(format!("{:?}", err))),
                Ok(derived_root) => {
                    // Check if the derived proof matches the one from the snapshot
                    Ok(snaptshot_root == derived_root)
                }
            }
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// We test if the default constructor does its job.
        #[ink::test]
        fn test_constructor() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let minimum_endorsements: u16 = 1;
            let validators: Vec<AccountId> = vec![admin];

            let validator = Validator::new(
                admin.clone(),
                minimum_endorsements.clone(),
                validators.clone(),
            );
            assert_eq!(validator.config.governance_address, admin);
            assert_eq!(validator.config.minimum_endorsements, minimum_endorsements);
            assert_eq!(validator.config.validators, validators);
            assert_eq!(validator.current_snapshot, 1);
        }

        /// We test a simple use case of our contract.
        #[ink::test]
        fn test_submit_root() {
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let admin = accounts.alice;
            let minimum_endorsements: u16 = 1;
            let validators: Vec<AccountId> = vec![admin];

            let mut validator = Validator::new(admin, minimum_endorsements, validators);

            let snapshot_number = 1;
            let snapshot_root = [0; 32];

            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(admin);
            validator.submit_root(snapshot_number, snapshot_root);

            assert_eq!(validator.current_snapshot, 2);
            assert_eq!(validator.snapshot_submissions_accounts.len(), 1);
            assert_eq!(validator.root.get(snapshot_number), Some(snapshot_root));

            assert_eq!(validator.validate_block_state_root(), Some(snapshot_root));
        }
    }
}
