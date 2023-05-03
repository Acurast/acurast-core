use alloc::string::String;
use codec::alloc;
use once_cell::race::OnceBox;
use sp_core::H256;
use sp_runtime::traits::Keccak256;
use sp_std::prelude::*;
use sp_std::vec;
use tezos_core::types::encoded::{Encoded, P256PublicKey, PublicKey};
use tezos_core::types::number::Nat;
use tezos_core::Error as TezosCoreError;
use tezos_michelson::micheline::Micheline;
use tezos_michelson::michelson::data;
use tezos_michelson::michelson::data::String as TezosString;
use tezos_michelson::michelson::types::{address, bytes, nat, pair, string};
use tezos_michelson::Error as TezosMichelineError;

use pallet_acurast_marketplace::PubKeyBytes;

use crate::types::TargetChainConfig;
use crate::Action;
use crate::Leaf;
use crate::{LeafEncoder, RawAction};

/// The [`LeafEncoder`] for Tezos using Micheline/Michelson encoding/packing.
pub struct TezosEncoder();

impl LeafEncoder for TezosEncoder {
    type Error = TezosMichelineError;

    /// Encodes the given message for Tezos.
    ///
    /// Message gets encoded/packed as
    ///
    /// ```text
    /// RawMessage {
    ///     id: u32,
    ///     action: crate::RawAction,
    ///     payload: Vec<u8>,
    /// }
    /// ```
    ///
    /// where payload is dependent on `action` and encoded as a sequence of the [`Action`] variants' bodies, e.g.
    /// `[JobIdSequence, Vec<TezosAddressBytes>]` in the case of [`Action::AssignJob`].
    fn encode(message: &Leaf) -> Result<Vec<u8>, Self::Error> {
        let raw_action: RawAction = (&message.action).into();
        let action_str: &'static str = raw_action.into();
        let data = data::pair(vec![
            data::int(message.id as i64),
            data::try_string(action_str)?,
            data::bytes(match &message.action {
                Action::AssignJob(job_id, processor_address) => {
                    let data = data::pair(vec![
                        data::nat(Nat::from_integer(*job_id)),
                        data::string(TezosString::from_string(processor_address.to_owned())?),
                    ]);
                    Micheline::pack(data, Some(assign_payload_schema()))
                }
                Action::Noop => Ok(Default::default()),
            }?),
        ]);

        Micheline::pack(data, Some(message_schema()))
    }
}

#[cfg_attr(rustfmt, rustfmt::skip)]
fn message_schema() -> &'static Micheline {
    static MESSAGE_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    MESSAGE_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            // id
            nat(),
            // action
            string(),
            // payload
            bytes(),
        ]);
        Box::new(schema)
    })
}

#[cfg_attr(rustfmt, rustfmt::skip)]
fn assign_payload_schema() -> &'static Micheline {
    static ASSIGN_PAYLOAD_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    ASSIGN_PAYLOAD_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            // job_id_seq
            nat(),
            // processor_address
            address()
        ]);
        Box::new(schema)
    })
}

pub struct DefaultTezosConfig;

impl TargetChainConfig for DefaultTezosConfig {
    type TargetChainEncoder = TezosEncoder;
    type Hasher = Keccak256;
    type Hash = H256;
}

/// Helper function to covert the BoundedVec [`PubKeyBytes`] to a Tezos [`String`].
pub fn p256_pub_key_to_address(pub_key: &PubKeyBytes) -> Result<String, TezosCoreError> {
    let key = P256PublicKey::from_bytes(pub_key)?;
    let key: PublicKey = key.into();
    key.bs58_address()
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use crate::stub::tezos_account_id;
    use crate::{tezos, Message};

    use super::*;

    #[test]
    fn test_unpack() -> Result<(), <TezosEncoder as LeafEncoder>::Error> {
        let encoded = tezos::TezosEncoder::encode(&Message {
            id: 5,
            action: Action::AssignJob(4, tezos_account_id()),
        })?;

        let expected = &hex!("05070700050707010000001441535349474e5f4a4f425f50524f434553534f520a0000002005070700040a000000160000eaeec9ada5305ad61fc452a5ee9f7d4f55f80467");
        assert_eq!(expected, &*encoded);
        Ok(())
    }
}
