#![allow(dead_code)]

use codec::Encode;
use frame_support::{
    parameter_types,
    sp_runtime::{traits::AccountIdConversion, AccountId32, MultiSignature},
    weights::Weight,
    PalletId,
};
use hex_literal::hex;
use sp_core::{sr25519, Pair};
#[cfg(feature = "std")]
pub type UncheckedExtrinsic<T> = frame_system::mocking::MockUncheckedExtrinsic<T>;
#[cfg(feature = "std")]
pub type Block<T> = frame_system::mocking::MockBlock<T>;
pub type AssetId = u128;
pub type Balance = u128;
pub type AccountId = AccountId32;
pub type BlockNumber = u32;

pub const SEED: u32 = 1337;
pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;
pub const UNIT: Balance = 1_000_000;
pub const MILLIUNIT: Balance = UNIT / 1_000;
pub const MICROUNIT: Balance = UNIT / 1_000_000;
pub const INITIAL_BALANCE: u128 = UNIT * 100;

parameter_types! {
    pub const BlockHashCount: BlockNumber = 2400;
    pub const RootAccountId: AccountId = alice_account_id();
}
parameter_types! {
    pub BlockWeights: frame_system::limits::BlockWeights = frame_system::limits::BlockWeights::simple_max(Weight::from_ref_time(1024));
    pub const MinimumPeriod: u64 = 2000;
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}
parameter_types! {
    pub const MaxReserves: u32 = 50;
    pub const MaxLocks: u32 = 50;
    pub const AcurastPalletId: PalletId = PalletId(*b"acrstpid");
    pub const ReportTolerance: u64 = 12000;
}

pub fn pallet_assets_account() -> AccountId {
    AcurastPalletId::get().into_account_truncating()
}

pub fn processor_account_id() -> AccountId {
    hex!("b8bc25a2b4c0386b8892b43e435b71fe11fa50533935f027949caf04bcce4694").into()
}

pub const fn alice_account_id() -> AccountId {
    AccountId32::new([0u8; 32])
}

pub const fn bob_account_id() -> AccountId {
    AccountId32::new([1u8; 32])
}

pub const fn charlie_account_id() -> AccountId {
    AccountId32::new([2u8; 32])
}

pub const fn dave_account_id() -> AccountId {
    AccountId32::new([3u8; 32])
}

pub const fn eve_account_id() -> AccountId {
    AccountId32::new([4u8; 32])
}

pub fn generate_account() -> (sr25519::Pair, AccountId) {
    let (pair, _) = sr25519::Pair::generate();
    let account_id: AccountId = pair.public().into();

    (pair, account_id)
}

pub fn generate_signature(
    signer: &sr25519::Pair,
    account: &AccountId,
    timestamp: u128,
    counter: u64,
) -> MultiSignature {
    let message = [account.encode(), timestamp.encode(), counter.encode()].concat();
    signer.sign(&message).into()
}

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn sign() {
//         let pair = sr25519::Pair::from_string("//Bob", None).unwrap();
//         let signature = pair.sign(&[0]);
//         println!("{:?}", signature)
//     }
// }
