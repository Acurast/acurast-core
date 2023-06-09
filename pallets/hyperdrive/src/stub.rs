#![allow(dead_code)]

use hex_literal::hex;
use sp_core::H256;
use sp_runtime::AccountId32;

pub fn alice_account_id() -> AccountId32 {
    [0; 32].into()
}
pub fn bob_account_id() -> AccountId32 {
    [1; 32].into()
}
pub const HASH: H256 = H256(hex!(
    "a3f18e4c6f0cdd0d8666f407610351cacb9a263678cf058294be9977b69f2cb3"
));
