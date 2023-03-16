use std::marker::PhantomData;

use derive_more::{Display, From};
use frame_support::once_cell::race::OnceBox;
use frame_support::Parameter;
use sp_core::bounded::BoundedVec;
use sp_core::ConstU32;
use sp_runtime::traits::Member;
use sp_std::prelude::*;
use sp_std::str::FromStr;
use tezos_core::Error as TezosCoreError;
use tezos_core::types::encoded::Address as TezosAddress;
use tezos_michelson::Error as TezosMichelineError;
use tezos_michelson::micheline::Micheline;
use tezos_michelson::micheline::primitive_application::PrimitiveApplication;
use tezos_michelson::michelson::data;
use tezos_michelson::michelson::data::{Bytes, Data, Int, Pair, Sequence, try_int, try_string};
use tezos_michelson::michelson::types::{
    address, bool as bool_type, bytes, nat, option, pair, set, string,
};

use pallet_acurast::{JobIdSequence, JobRegistration, MultiOrigin, Schedule};
use pallet_acurast_marketplace::{JobRequirements, PlannedExecution, RegistrationExtra};

use crate::{Config, ParsedAction};
use crate::Error;
use crate::types::{MessageParser, RawAction};

pub struct TezosParser<Reward, Balance, ParsableAccountId, AccountId, Extra>(
    PhantomData<(Reward, Balance, ParsableAccountId, AccountId, Extra)>,
);

impl<Reward, Balance, ParsableAccountId, AccountId, Extra> MessageParser<AccountId, Extra>
    for TezosParser<Reward, Balance, ParsableAccountId, AccountId, Extra>
where
    ParsableAccountId: FromStr + Into<AccountId>,
    Extra: From<RegistrationExtra<Reward, Balance, AccountId>>,
    Reward: Parameter + Member + TryFrom<Vec<u8>>,
    Balance: From<u128>,
{
    type Error = ValidationError;
    fn parse(encoded: &[u8]) -> Result<ParsedAction<AccountId, Extra>, ValidationError> {
        let (action, origin, payload) = parse_message(encoded)?;

        Ok(match action {
            RawAction::RegisterJob => {
                let payload: Vec<u8> = (&payload).into();
                let (job_id_sequence, registration) = parse_job_registration_payload::<
                    Reward,
                    Balance,
                    ParsableAccountId,
                    AccountId,
                    Extra,
                >(payload.as_slice())?;

                ParsedAction::RegisterJob(
                    (to_multi_origin(&origin)?, job_id_sequence),
                    registration,
                )
            }
        })
    }
}

impl<T: Config, I> From<ValidationError> for Error<T, I> {
    fn from(_: ValidationError) -> Self {
        Error::<T, I>::MessageParsingFailed
    }
}

#[cfg_attr(rustfmt, rustfmt::skip)]
fn message_schema() -> &'static Micheline {
    static MESSAGE_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    MESSAGE_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            string(),
            address(),
            bytes(),
        ]);
        Box::new(schema)
    })
}

/// Parses an encoded message from Tezos representing an action into a tuple `(ACTION, ORIGIN, PAYLOAD)`.
///
/// # Example
/// A message to register a job could look like:
///
/// ```txt
/// PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(String(String("REGISTER_JOB"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Bytes(Bytes("0x00008a8584be3718453e78923713a6966202b05f99c6"))), Literal(Bytes(Bytes("0x050707030a0707030607070a0000001601000000000000000000000000000000000000000000070707070000070703060707030607070a00000035697066733a2f2f516d64484c6942596174626e6150645573544d4d4746574534326353414a43485937426f3741445832636444656100010707000207070001070700010707070700b0d403070700bfe6d987d86107070098e4030707000000bf9a9f87d86107070a00000035697066733a2f2f516d64484c6942596174626e6150645573544d4d4746574534326353414a43485937426f374144583263644465610001")))]), annots: None })]), annots: None })
/// ```
fn parse_message(encoded: &[u8]) -> Result<(RawAction, TezosAddress, Bytes), ValidationError> {
    let unpacked: Micheline = Micheline::unpack(encoded, Some(message_schema()))
        .map_err(|e| ValidationError::TezosMicheline(e))?;

    let p: PrimitiveApplication = unpacked.try_into()?;
    let pair: Pair = p.try_into()?;

    let values = pair.flatten().values;
    if values.len() != 3 {
        Err(ValidationError::InvalidMessage)?;
    }
    let mut iter = values.into_iter();

    let action = {
        let action: data::String = iter
            .next()
            .ok_or(ValidationError::MissingField("ACTION".to_string()))?
            .try_into()?;
        RawAction::from_str(action.to_str()).map_err(|_| ValidationError::InvalidAction)?
    };
    let origin: TezosAddress = try_address(
        iter.next()
            .ok_or(ValidationError::MissingField("ORIGIN".to_string()))?,
    )?;
    let body: Bytes = iter
        .next()
        .ok_or(ValidationError::MissingField("PAYLOAD".to_string()))?
        .try_into()?;

    Ok((action, origin, body))
}

/// The structure of a [`RawAction::RegisterJob`] action before flattening:
///
/// ```txt
/// sp.TRecord(
///     allowOnlyVerifiedSources=sp.TBool,
///     allowedSources=sp.TOption(sp.TSet(sp.TString)),
///     destination=sp.TAddress,
///     extra=sp.TRecord(
///         expectedFulfillmentFee=sp.TNat,
///         requirements=sp.TRecord(
///             instantMatch=sp.TOption(
///                 sp.TSet(
///                     sp.TRecord(
///                         source=sp.TString,
///                         startDelay=sp.TNat,
///                     )
///                 )
///             ),
///             minReputation=sp.TOption(sp.TNat),
///             reward=sp.TBytes,
///             slots=sp.TNat,
///         ).right_comb(),
///     ).right_comb(),
///     jobId=sp.TNat,
///     memory=sp.TNat,
///     networkRequests=sp.TNat,
///     schedule=sp.TRecord(
///         duration=sp.TNat,
///         endTime=sp.TNat,
///         interval=sp.TNat,
///         maxStartDelay=sp.TNat,
///         startTime=sp.TNat,
///     ).right_comb(),
///     script=sp.TBytes,
///     storage=sp.TNat,
/// ).right_comb()
/// ```
#[cfg_attr(rustfmt, rustfmt::skip)]
fn registration_payload_schema() -> &'static Micheline {
    static REGISTRATION_PAYLOAD_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    REGISTRATION_PAYLOAD_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            bool_type(),
            option(set(string())),
            address(),
            pair(vec![
                nat(),
                pair(vec![
                    option(set(pair(vec![
                        string(),
                        nat()
                    ]))),
                    option(nat()),
                    bytes(),
                    nat(),
                ])
            ]),
            nat(),
            nat(),
            nat(),
            pair(vec![
                nat(),
                nat(),
                nat(),
                nat(),
                nat(),
            ]),
            bytes(),
            nat(),
        ]);
        Box::new(schema)
    })
}

/// Parses an encoded [`RawAction::RegisterJob`] action's payload into [`JobRegistration`].
///
/// # Example
/// A message's payload to register a job could look like:
///
/// ```txt
/// PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([PrimitiveApplication(PrimitiveApplication { prim: "True", args: None, annots: None }), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([PrimitiveApplication(PrimitiveApplication { prim: "Some", args: Some([Sequence(Sequence([Literal(String(String("5DxbTWE4FkSdCQ1D6mJDN2nqcaVw7MaKqwjvjDGRdYenKk2M")))]))]), annots: None }), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Bytes(Bytes("0x01000000000000000000000000000000000000000000"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Int(Int("0"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([PrimitiveApplication(PrimitiveApplication { prim: "Some", args: Some([Sequence(Sequence([PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(String(String("5DxbTWE4FkSdCQ1D6mJDN2nqcaVw7MaKqwjvjDGRdYenKk2M"))), Literal(Int(Int("0")))]), annots: None })]))]), annots: None }), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([PrimitiveApplication(PrimitiveApplication { prim: "None", args: None, annots: None }), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Bytes(Bytes("0xff"))), Literal(Int(Int("1")))]), annots: None })]), annots: None })]), annots: None })]), annots: None }), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Int(Int("4"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Int(Int("1"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Int(Int("1"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Int(Int("30000"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Int(Int("1678266546623"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Int(Int("31000"))), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Int(Int("0"))), Literal(Int(Int("1678266066623")))]), annots: None })]), annots: None })]), annots: None })]), annots: None }), PrimitiveApplication(PrimitiveApplication { prim: "Pair", args: Some([Literal(Bytes(Bytes("0x697066733a2f2f516d64484c6942596174626e6150645573544d4d4746574534326353414a43485937426f37414458326364446561"))), Literal(Int(Int("1")))]), annots: None })]), annots: None })]), annots: None })]), annots: None })]), annots: None })]), annots: None })]), annots: None })]), annots: None })]), annots: None })
/// ```
fn parse_job_registration_payload<Reward, Balance, ParsableAccountId, AccountId, Extra>(
    encoded: &[u8],
) -> Result<(JobIdSequence, JobRegistration<AccountId, Extra>), ValidationError>
where
    ParsableAccountId: FromStr + Into<AccountId>,
    Extra: From<RegistrationExtra<Reward, Balance, AccountId>>,
    Reward: Parameter + Member + TryFrom<Vec<u8>>,
    Balance: From<u128>,
{
    let unpacked: Micheline = Micheline::unpack(encoded, Some(registration_payload_schema()))
        .map_err(|e| ValidationError::TezosMicheline(e))?;

    let p: PrimitiveApplication = unpacked.try_into()?;
    let pair: Pair = p.try_into()?;

    let values = pair.flatten().values;
    if values.len() != 18 {
        Err(ValidationError::InvalidMessage)?;
    }
    let mut iter = values.into_iter();

    let allow_only_verified_sources: bool = try_bool(iter.next().ok_or(
        ValidationError::MissingField("allowOnlyVerifiedSources".to_string()),
    )?)?;
    let allowed_sources = try_option(
        iter.next()
            .ok_or(ValidationError::MissingField("allowedSources".to_string()))?,
        |value| {
            try_sequence(value, |source| {
                let s: data::String = source.try_into()?;
                Ok(ParsableAccountId::from_str(s.to_str())
                    .map_err(|_| ValidationError::AddressParsing)?
                    .into())
            })
        },
    )?;

    let destination = {
        let address = try_address(
            iter.next()
                .ok_or(ValidationError::MissingField("destination".to_string()))?,
        )?;
        to_multi_origin(&address)?
    };
    let expected_fulfillment_fee = {
        let v: Int = try_int(iter.next().ok_or(ValidationError::MissingField(
            "expectedFulfillmentFee".to_string(),
        ))?)?;
        let v: u128 = v.to_integer()?;
        v.into()
    };
    let instant_match = try_option(
        iter.next()
            .ok_or(ValidationError::MissingField("instantMatch".to_string()))?,
        |value| {
            let sources = try_sequence(value, |planned_execution| {
                let pair: Pair = planned_execution.try_into()?;
                let values = pair.flatten().values;
                if values.len() != 2 {
                    Err(ValidationError::InvalidMessage)?;
                }
                let mut iter = values.into_iter();

                let source = {
                    let s: data::String = iter
                        .next()
                        .ok_or(ValidationError::MissingField("source".to_string()))?
                        .try_into()?;
                    Ok::<AccountId, ValidationError>(
                        ParsableAccountId::from_str(s.to_str())
                            .map_err(|_| ValidationError::AddressParsing)?
                            .into(),
                    )
                }?;

                let start_delay = {
                    let v: Int = try_int(
                        iter.next()
                            .ok_or(ValidationError::MissingField("startDelay".to_string()))?,
                    )?;
                    v.to_integer()?
                };

                Ok(PlannedExecution {
                    source,
                    start_delay,
                })
            })?;

            Ok(sources)
        },
    )?;
    let min_reputation = try_option(
        iter.next()
            .ok_or(ValidationError::MissingField("minReputation".to_string()))?,
        |value| {
            let v: Int = try_int(value)?;
            Ok(v.to_integer()?)
        },
    )?;
    let reward = {
        let reward: Bytes = iter
            .next()
            .ok_or(ValidationError::MissingField("reward".to_string()))?
            .try_into()?;
        let reward: Vec<u8> = (&reward).into();
        reward
            .try_into()
            .map_err(|_| ValidationError::InvalidReward)?
    };
    let slots = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("slots".to_string()))?,
        )?;
        v.to_integer()?
    };
    let job_id = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("jobId".to_string()))?,
        )?;
        v.to_integer()?
    };
    let memory = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("memory".to_string()))?,
        )?;
        v.to_integer()?
    };
    let network_requests = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("networkRequests".to_string()))?,
        )?;
        v.to_integer()?
    };
    let duration = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("duration".to_string()))?,
        )?;
        v.to_integer()?
    };
    let end_time = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("endTime".to_string()))?,
        )?;
        v.to_integer()?
    };
    let interval = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("interval".to_string()))?,
        )?;
        v.to_integer()?
    };
    let max_start_delay = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("maxStartDelay".to_string()))?,
        )?;
        v.to_integer()?
    };
    let start_time = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("startTime".to_string()))?,
        )?;
        v.to_integer()?
    };

    let script = {
        let script: Bytes = iter
            .next()
            .ok_or(ValidationError::MissingField("script".to_string()))?
            .try_into()?;
        let script: Vec<u8> = (&script).into();
        script
            .try_into()
            .map_err(|_| ValidationError::ScriptOutOfBounds)?
    };
    let storage = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField("storage".to_string()))?,
        )?;
        v.to_integer()?
    };

    let extra: Extra = RegistrationExtra {
        destination,
        parameters: None,
        requirements: JobRequirements {
            slots,
            reward,
            min_reputation,
            instant_match: instant_match,
        },
        expected_fulfillment_fee,
    }
    .into();
    Ok((
        job_id,
        JobRegistration {
            script,
            allowed_sources,
            allow_only_verified_sources,
            schedule: Schedule {
                duration,
                start_time,
                end_time,
                interval,
                max_start_delay,
            },
            memory,
            network_requests,
            storage,
            extra: extra,
        },
    ))
}

fn to_multi_origin<AccountId>(
    address: &TezosAddress,
) -> Result<MultiOrigin<AccountId>, ValidationError> {
    let v: Vec<u8> = match &address {
        TezosAddress::Implicit(a) => a.try_into()?,
        TezosAddress::Originated(a) => a.try_into()?,
    };
    let v = BoundedVec::<u8, ConstU32<36>>::try_from(v.to_owned())
        .map_err(|_| ValidationError::TezosAddressOutOfBounds)?;
    Ok(MultiOrigin::Tezos(v))
}

#[derive(Display, Debug, From)]
pub enum ValidationError {
    TezosMicheline(TezosMichelineError),
    TezosCore(TezosCoreError),
    InvalidMessage,
    InvalidAction,
    ScriptOutOfBounds,
    TezosAddressOutOfBounds,
    InvalidReward,
    MissingField(String),
    InvalidBool,
    InvalidOption,
    AddressParsing,
}

/// Utility function to parse a tezos [`Bool`] into a Rust bool.
fn try_bool(value: Data) -> Result<bool, ValidationError> {
    match value {
        Data::True(_) => Ok(true),
        Data::False(_) => Ok(false),
        _ => Err(ValidationError::InvalidBool),
    }
}

/// Utility function to parse a tezos [`Bool`] into a Rust bool.
fn try_address(value: Data) -> Result<TezosAddress, ValidationError> {
    let origin: data::String = try_string(value)?;
    let origin: TezosAddress = origin.to_str().try_into()?;
    Ok(origin)
}

/// Utility function to parse a tezos [`MichelsonOption`] into a Rust Option, applying a conversion operation once to *Some* value.
fn try_option<R, O: FnOnce(Data) -> Result<R, ValidationError>>(
    value: Data,
    op: O,
) -> Result<Option<R>, ValidationError> {
    match value {
        Data::Some(v) => Ok(Some(op(*v.value)?)),
        Data::None(_) => Ok(None),
        _ => Err(ValidationError::InvalidOption),
    }
}

/// Utility function to parse a tezos [`Sequence`] into a [`Vec`], applying a conversion operation to each item of the sequence.
fn try_sequence<R, O: Fn(Data) -> Result<R, ValidationError>>(
    value: Data,
    op: O,
) -> Result<Vec<R>, ValidationError> {
    let s: Sequence = value.try_into()?;
    s.into_values().into_iter().map(|item| op(item)).collect()
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use pallet_acurast::{JobRegistration, Script};

    use crate::mock::*;

    use super::*;

    #[test]
    fn test_unpack() -> Result<(), ValidationError> {
        let encoded = &hex!("050707010000000c52454749535445525f4a4f4207070a0000001600008a8584be3718453e78923713a6966202b05f99c60a00000122050707030a07070509020000003501000000303544786254574534466b536443513144366d4a444e326e7163615677374d614b71776a766a4447526459656e4b6b324d07070a0000001601000000000000000000000000000000000000000000070707070000070705090200000039070701000000303544786254574534466b536443513144366d4a444e326e7163615677374d614b71776a766a4447526459656e4b6b324d00000707030607070a00000001ff00010707000407070001070700010707070700b0d403070700bfe6d987d86107070098e4030707000000bf9a9f87d86107070a00000035697066733a2f2f516d64484c6942596174626e6150645573544d4d4746574534326353414a43485937426f374144583263644465610001");
        let (action, origin, payload) = parse_message(encoded)?;
        assert_eq!(RawAction::RegisterJob, action);
        let exp: TezosAddress = "tz1YGTtd1hqGYTYKtcWSXYKSgCj5hvjaTPVd".try_into().unwrap();
        assert_eq!(exp, origin);

        let payload: Vec<u8> = (&payload).into();
        let (job_id, registration) = parse_job_registration_payload::<
            _,
            _,
            <Test as Config>::ParsableAccountId,
            <Test as frame_system::Config>::AccountId,
            _,
        >(payload.as_slice())?;
        let expected = JobRegistration::<<Test as frame_system::Config>::AccountId, _> {
            script: Script::try_from(vec![
                105, 112, 102, 115, 58, 47, 47, 81, 109, 100, 72, 76, 105, 66, 89, 97, 116, 98,
                110, 97, 80, 100, 85, 115, 84, 77, 77, 71, 70, 87, 69, 52, 50, 99, 83, 65, 74, 67,
                72, 89, 55, 66, 111, 55, 65, 68, 88, 50, 99, 100, 68, 101, 97,
            ])
            .unwrap(),
            allowed_sources: Some(vec![hex![
                "53cf73c65e36ec0bf3d7539780e83febd2d1b01de0df4f6bb7a95157715f2196"
            ]
            .into()]),
            allow_only_verified_sources: true,
            schedule: Schedule {
                duration: 30000,
                start_time: 1678266066623,
                end_time: 1678266546623,
                interval: 31000,
                max_start_delay: 0,
            },
            memory: 1,
            network_requests: 1,
            storage: 1,
            extra: RegistrationExtra {
                destination: MultiOrigin::Tezos(
                    BoundedVec::<u8, ConstU32<36>>::try_from([0; 21].to_vec()).unwrap(),
                ),
                parameters: None,
                requirements: JobRequirements {
                    slots: 1,
                    reward: MockAsset {
                        id: 5,
                        amount: 10000,
                    },
                    min_reputation: None,
                    instant_match: Some(vec![PlannedExecution {
                        source: hex![
                            "53cf73c65e36ec0bf3d7539780e83febd2d1b01de0df4f6bb7a95157715f2196"
                        ]
                        .into(),
                        start_delay: 0,
                    }]),
                },
                expected_fulfillment_fee: 0,
            },
        };
        assert_eq!(expected, registration);
        assert_eq!(4, job_id);
        Ok(())
    }
}
