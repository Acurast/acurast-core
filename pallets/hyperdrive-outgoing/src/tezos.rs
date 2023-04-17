use once_cell::race::OnceBox;
use sp_core::H256;
use sp_runtime::traits::Keccak256;
use sp_std::prelude::*;
use sp_std::vec;
use tezos_michelson::micheline::Micheline;
use tezos_michelson::michelson::data;
use tezos_michelson::michelson::types::{bytes, list, nat, pair, string};
use tezos_michelson::Error as TezosMichelineError;

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
                Action::AssignJob(job_id, processor_addresses) => {
                    let data = data::pair(vec![
                        data::bytes(job_id.to_be_bytes().as_slice()),
                        data::sequence(
                            processor_addresses
                                .iter()
                                .map(|a| data::bytes(a.as_slice()))
                                .collect(),
                        ),
                    ]);
                    Micheline::pack(data, Some(assign_payload_schema()))
                }
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
            // job_id_seq integer as a single big-endian byte
            bytes(),
            // processor_addresses
            list(
                bytes()
            ),
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
            action: Action::AssignJob(4, vec![tezos_account_id()]),
        })?;

        let expected = &hex!("05070700050707010000000641535349474e0a000000460507070a000000100000000000000000000000000000000402000000290a00000024747a316834457347756e48325565315432754e73386d664b5a38585a6f516a693348634b");
        assert_eq!(expected, &*encoded);
        Ok(())
    }
}
