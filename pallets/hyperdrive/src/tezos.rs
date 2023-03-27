use core::marker::PhantomData;

#[cfg(feature = "std")]
use derive_more::Error as DError;
use derive_more::{Display, From};
use frame_support::Parameter;
use once_cell::race::OnceBox;
use sp_core::bounded::BoundedVec;
use sp_runtime::traits::Member;
use sp_std::prelude::*;
use sp_std::str::FromStr;
use sp_std::vec;
use tezos_core::types::encoded::Address as TezosAddress;
use tezos_core::Error as TezosCoreError;
use tezos_michelson::micheline::primitive_application::PrimitiveApplication;
use tezos_michelson::michelson::data::{
    self, try_bytes, try_int, try_string, Bytes, Data, Int, Nat, Pair, Sequence,
};
use tezos_michelson::michelson::types::{
    address, bool as bool_type, bytes, nat, option, pair, set, string,
};
use tezos_michelson::Error as TezosMichelineError;
use tezos_michelson::{
    micheline::{primitive_application, Micheline},
    michelson::ComparableTypePrimitive,
};

use pallet_acurast::{JobIdSequence, JobModule, JobRegistration, MultiOrigin, Schedule, CU32};
use pallet_acurast_marketplace::{
    JobRequirements, MultiDestination, PlannedExecution, RegistrationExtra,
};

use crate::types::{MessageParser, RawAction};
use crate::RewardParser;
use crate::{MessageIdentifier, ParsedAction};

pub struct TezosParser<Reward, Balance, ParsableAccountId, AccountId, Extra, AssetParser>(
    PhantomData<(
        Reward,
        Balance,
        ParsableAccountId,
        AccountId,
        Extra,
        AssetParser,
    )>,
);

impl<Reward, Balance, ParsableAccountId, AccountId, Extra, AssetParser>
    MessageParser<Reward, AccountId, Extra>
    for TezosParser<Reward, Balance, ParsableAccountId, AccountId, Extra, AssetParser>
where
    ParsableAccountId: TryFrom<Vec<u8>> + Into<AccountId>,
    Extra: From<RegistrationExtra<Reward, Balance, AccountId>>,
    Reward: Parameter + Member,
    Balance: From<u128>,
    AssetParser: RewardParser<Reward>,
{
    type Error = ValidationError;
    type AssetParser = AssetParser;

    /// Parses an encoded key from Tezos representing a message identifier.
    fn parse_key(encoded: &[u8]) -> Result<MessageIdentifier, Self::Error> {
        let schema = primitive_application(ComparableTypePrimitive::Nat).into();
        let micheline: Micheline = Micheline::unpack(encoded, Some(&schema))
            .map_err(|e| ValidationError::TezosMicheline(e))?;

        let value: Nat = micheline.try_into()?;
        value.to_integer().map_err(|_| ValidationError::InvalidKey)
    }

    fn parse_value(encoded: &[u8]) -> Result<ParsedAction<AccountId, Extra>, ValidationError> {
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
                    AssetParser,
                >(payload.as_slice())?;

                ParsedAction::RegisterJob(
                    (
                        MultiOrigin::Tezos(bounded_address(&origin)?),
                        job_id_sequence,
                    ),
                    registration,
                )
            }
        })
    }
}

#[cfg_attr(rustfmt, rustfmt::skip)]
fn message_schema() -> &'static Micheline {
    static MESSAGE_SCHEMA: OnceBox<Micheline> = OnceBox::new();
    MESSAGE_SCHEMA.get_or_init(|| {
        let schema: Micheline = pair(vec![
            // ACTION NAME
            string(),
            // TEZOS ORIGIN
            address(),
            // ACTION PAYLOAD
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
            .ok_or(ValidationError::MissingField(FieldError::ACTION))?
            .try_into()?;
        RawAction::from_str(action.to_str()).map_err(|_| ValidationError::InvalidAction)?
    };
    let origin: TezosAddress = try_address(
        iter.next()
            .ok_or(ValidationError::MissingField(FieldError::ORIGIN))?,
    )?;
    let body: Bytes = try_bytes(
        iter.next()
            .ok_or(ValidationError::MissingField(FieldError::PAYLOAD))?,
    )?;

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
///     requiredModules = sp.TSet(sp.TNat),
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
            // allow_only_verified_sources
            bool_type(),
            // allowed_sources
            option(set(bytes())),
            // destination
            address(),
            // RegistrationExtra
            pair(vec![
                // expected_fulfillment_fee
                nat(),
                // instant_match
                pair(vec![
                    option(
                        // PlannedExecutions
                        set(pair(vec![
                        // source
                        bytes(),
                        // start_delay
                        nat()
                    ]))),
                    // min_reputation
                    option(nat()),
                    // reward
                    bytes(),
                    // slots
                    nat(),
                ])
            ]),
            // job_id
            nat(),
            // memory
            nat(),
            // network_requests
            nat(),
            // required_modules
            set(nat()),
            // schedule
            pair(
                // Schedules
                vec![
                // duration
                nat(),
                // end_time
                nat(),
                // interval
                nat(),
                // max_start_delay
                nat(),
                // start_time
                nat(),
            ]),
            // script
            bytes(),
            // storage
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
fn parse_job_registration_payload<
    Reward,
    Balance,
    ParsableAccountId,
    AccountId,
    Extra,
    AssetParser,
>(
    encoded: &[u8],
) -> Result<(JobIdSequence, JobRegistration<AccountId, Extra>), ValidationError>
where
    ParsableAccountId: TryFrom<Vec<u8>> + Into<AccountId>,
    Extra: From<RegistrationExtra<Reward, Balance, AccountId>>,
    Reward: Parameter + Member,
    Balance: From<u128>,
    AssetParser: RewardParser<Reward>,
{
    let unpacked: Micheline = Micheline::unpack(encoded, Some(registration_payload_schema()))
        .map_err(|e| ValidationError::TezosMicheline(e))?;

    let p: PrimitiveApplication = unpacked.try_into()?;
    let pair: Pair = p.try_into()?;

    let values = pair.flatten().values;
    let mut iter = values.into_iter();

    // !!! [IMPORTANT]: The values need to be decoded alphabetically !!!

    let allow_only_verified_sources: bool = try_bool(iter.next().ok_or(
        ValidationError::MissingField(FieldError::AllowOnlyVerifiedSources),
    )?)?;
    let allowed_sources = try_option(
        iter.next()
            .ok_or(ValidationError::MissingField(FieldError::AllowedSources))?,
        |value| {
            try_sequence(value, |source| {
                let s: Vec<u8> = (&try_bytes::<_, Bytes, _>(source)?).into();
                let parsed: ParsableAccountId =
                    s.try_into().map_err(|_| ValidationError::AddressParsing)?;
                Ok(parsed.into())
            })
        },
    )?;

    let destination = {
        let address = try_address(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::Destination))?,
        )?;
        MultiDestination::Tezos(bounded_address(&address)?)
    };
    let expected_fulfillment_fee = {
        let v: Int = try_int(iter.next().ok_or(ValidationError::MissingField(
            FieldError::ExpectedFulfillmentFee,
        ))?)?;
        let v: u128 = v.to_integer()?;
        v.into()
    };
    let instant_match = try_option(
        iter.next()
            .ok_or(ValidationError::MissingField(FieldError::InstantMatch))?,
        |value| {
            let sources = try_sequence(value, |planned_execution| {
                let pair: Pair = planned_execution.try_into()?;
                let values = pair.flatten().values;
                if values.len() != 2 {
                    Err(ValidationError::InvalidMessage)?;
                }
                let mut iter = values.into_iter();

                let source = {
                    let s: Vec<u8> = (&try_bytes::<_, Bytes, _>(
                        iter.next()
                            .ok_or(ValidationError::MissingField(FieldError::Source))?,
                    )?)
                        .into();
                    let parsed: ParsableAccountId =
                        s.try_into().map_err(|_| ValidationError::AddressParsing)?;
                    Ok::<AccountId, ValidationError>(parsed.into())
                }?;

                let start_delay = {
                    let v: Int = try_int(
                        iter.next()
                            .ok_or(ValidationError::MissingField(FieldError::StartDelay))?,
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
            .ok_or(ValidationError::MissingField(FieldError::MinReputation))?,
        |value| {
            let v: Int = try_int(value)?;
            Ok(v.to_integer()?)
        },
    )?;
    let reward = {
        let reward: Bytes = try_bytes(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::Reward))?,
        )?;
        let reward: Vec<u8> = (&reward).into();
        AssetParser::parse(reward.to_vec()).map_err(|_| ValidationError::InvalidReward)?
    };
    let slots = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::Slots))?,
        )?;
        v.to_integer()?
    };
    let job_id = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::JobId))?,
        )?;
        v.to_integer()?
    };
    let memory = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::Memory))?,
        )?;
        v.to_integer()?
    };
    let network_requests = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::NetworkRequests))?,
        )?;
        v.to_integer()?
    };

    let required_modules_unparsed = iter
        .next()
        .ok_or(ValidationError::MissingField(FieldError::RequiredModules))?;
    let required_modules = try_sequence::<JobModule, _>(required_modules_unparsed, |module| {
        let value: Int = module.try_into()?;
        value
            .to_integer::<u32>()?
            .try_into()
            .map_err(|_| ValidationError::RequiredModulesParsing)
    })?
    .try_into()
    .map_err(|_| ValidationError::RequiredModulesParsing)?;

    let duration = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::Duration))?,
        )?;
        v.to_integer()?
    };
    let end_time = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::EndTime))?,
        )?;
        v.to_integer()?
    };
    let interval = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::Interval))?,
        )?;
        v.to_integer()?
    };
    let max_start_delay = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::MaxStartDelay))?,
        )?;
        v.to_integer()?
    };
    let start_time = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::StartTime))?,
        )?;
        v.to_integer()?
    };

    let script = {
        let script: Vec<u8> = (&try_bytes::<_, Bytes, _>(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::Script))?,
        )?)
            .into();
        script
            .try_into()
            .map_err(|_| ValidationError::ScriptOutOfBounds)?
    };
    let storage = {
        let v: Int = try_int(
            iter.next()
                .ok_or(ValidationError::MissingField(FieldError::Storage))?,
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
            instant_match,
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
            required_modules,
            extra,
        },
    ))
}

fn bounded_address(address: &TezosAddress) -> Result<BoundedVec<u8, CU32<36>>, ValidationError> {
    let v: Vec<u8> = match &address {
        TezosAddress::Implicit(a) => a.try_into()?,
        TezosAddress::Originated(a) => a.try_into()?,
    };
    Ok(BoundedVec::<u8, CU32<36>>::try_from(v.to_owned())
        .map_err(|_| ValidationError::TezosAddressOutOfBounds)?)
}

/// Errors returned by this crate.
#[derive(Display, Debug, From)]
#[cfg_attr(feature = "std", derive(DError))]
pub enum ValidationError {
    TezosMicheline(TezosMichelineError),
    TezosCore(TezosCoreError),
    InvalidKey,
    InvalidMessage,
    InvalidAction,
    ScriptOutOfBounds,
    TezosAddressOutOfBounds,
    InvalidReward,
    MissingField(FieldError),
    InvalidBool,
    InvalidOption,
    AddressParsing,
    RequiredModulesParsing,
}

#[derive(Display, Debug, From)]
#[cfg_attr(feature = "std", derive(DError))]
pub enum FieldError {
    ACTION,
    ORIGIN,
    PAYLOAD,
    AllowOnlyVerifiedSources,
    AllowedSources,
    Destination,
    ExpectedFulfillmentFee,
    InstantMatch,
    Source,
    StartDelay,
    MinReputation,
    Reward,
    Slots,
    JobId,
    Memory,
    NetworkRequests,
    Duration,
    EndTime,
    Interval,
    MaxStartDelay,
    RequiredModules,
    StartTime,
    Script,
    Storage,
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
    use crate::Config;

    use super::*;

    #[test]
    fn test_unpack() -> Result<(), ValidationError> {
        let encoded = &hex!("050707010000000c52454749535445525f4a4f4207070a000000160000eaeec9ada5305ad61fc452a5ee9f7d4f55f804670a0000010b050707030a0707050902000000250a00000020000000000000000000000000000000000000000000000000000000000000000007070a000000160100000000000000000000000000000000000000000007070707000007070509020000002907070a00000020111111111111111111111111111111111111111111111111111111111111111100000707030607070a00000001ff00010707000107070001070700010707020000000200000707070700b0d403070700bfe6d987d86107070098e4030707000000bf9a9f87d86107070a00000035697066733a2f2f516d64484c6942596174626e6150645573544d4d4746574534326353414a43485937426f374144583263644465610001");
        let (action, origin, payload) = parse_message(encoded)?;
        assert_eq!(RawAction::RegisterJob, action);
        let exp: TezosAddress = "tz1h4EsGunH2Ue1T2uNs8mfKZ8XZoQji3HcK".try_into().unwrap();
        assert_eq!(exp, origin);

        let payload: Vec<u8> = (&payload).into();
        let (job_id, registration): (
            JobIdSequence,
            JobRegistration<
                <Test as frame_system::Config>::AccountId,
                RegistrationExtra<
                    MockAsset,
                    AssetAmount,
                    <Test as frame_system::Config>::AccountId,
                >,
            >,
        ) = parse_job_registration_payload::<
            _,
            _,
            <Test as Config>::ParsableAccountId,
            <Test as frame_system::Config>::AccountId,
            _,
            SimpleAssetParser,
        >(payload.as_slice())?;
        let expected = JobRegistration::<<Test as frame_system::Config>::AccountId, _> {
            script: Script::try_from(vec![
                105, 112, 102, 115, 58, 47, 47, 81, 109, 100, 72, 76, 105, 66, 89, 97, 116, 98,
                110, 97, 80, 100, 85, 115, 84, 77, 77, 71, 70, 87, 69, 52, 50, 99, 83, 65, 74, 67,
                72, 89, 55, 66, 111, 55, 65, 68, 88, 50, 99, 100, 68, 101, 97,
            ])
            .unwrap(),
            allowed_sources: Some(vec![hex!(
                "0000000000000000000000000000000000000000000000000000000000000000"
            )
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
            required_modules: vec![JobModule::DataEncryption].try_into().unwrap(),
            extra: RegistrationExtra {
                destination: MultiDestination::Tezos(
                    BoundedVec::<u8, CU32<36>>::try_from([0; 21].to_vec()).unwrap(),
                ),
                parameters: None,
                requirements: JobRequirements {
                    slots: 1,
                    reward: MockAsset { id: 5, amount: 255 },
                    min_reputation: None,
                    instant_match: Some(vec![PlannedExecution {
                        source: hex![
                            "1111111111111111111111111111111111111111111111111111111111111111"
                        ]
                        .into(),
                        start_delay: 0,
                    }]),
                },
                expected_fulfillment_fee: 0,
            },
        };

        assert_eq!(expected, registration);
        assert_eq!(1, job_id);
        Ok(())
    }
}
