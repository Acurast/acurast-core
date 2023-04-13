#![allow(dead_code)]

use acurast_common::TezosAddressBytes;
use sp_core::*;
use sp_runtime::AccountId32;
use sp_std::prelude::*;
use tezos_core::types::encoded::{Address as TezosAddress, Encoded};

use crate::*;

#[cfg(feature = "std")]
pub type UncheckedExtrinsic<T> = frame_system::mocking::MockUncheckedExtrinsic<T>;
#[cfg(feature = "std")]
pub type Block<T> = frame_system::mocking::MockBlock<T>;
pub type AccountId = AccountId32;
pub type BlockNumber = u64;

pub fn tezos_account_id() -> TezosAddressBytes {
    let address: TezosAddress = "tz1h4EsGunH2Ue1T2uNs8mfKZ8XZoQji3HcK".try_into().unwrap();
    let address_bytes: Vec<u8> = address.value().into();
    TezosAddressBytes::truncate_from(address_bytes)
}

pub fn message(id: u128) -> Message {
    Message {
        id: id as u64,
        action: Action::AssignJob(id, vec![tezos_account_id()]),
    }
}

pub fn action(id: u128) -> Action {
    Action::AssignJob(id, vec![tezos_account_id()])
}
