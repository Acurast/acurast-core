#[cfg(feature = "attestation")]
mod bounded_attestation;

#[cfg(feature = "attestation")]
pub use bounded_attestation::*;

use frame_support::{
    pallet_prelude::*, sp_runtime::traits::MaybeDisplay, storage::bounded_vec::BoundedVec,
};
use sp_std::prelude::*;

pub(crate) const SCRIPT_PREFIX: &[u8] = b"ipfs://";
pub(crate) const SCRIPT_LENGTH: u32 = 53;

/// Type representing the utf8 bytes of a string containing the value of an ipfs url.
/// The ipfs url is expected to point to a script.
pub type Script = BoundedVec<u8, ConstU32<SCRIPT_LENGTH>>;

pub fn is_valid_script(script: &Script) -> bool {
    let script_len: u32 = script.len().try_into().unwrap_or(0);
    script_len == SCRIPT_LENGTH && script.starts_with(SCRIPT_PREFIX)
}

/// https://datatracker.ietf.org/doc/html/rfc5280#section-4.1.2.2
const SERIAL_NUMBER_MAX_LENGTH: u32 = 20;

pub type SerialNumber = BoundedVec<u8, ConstU32<SERIAL_NUMBER_MAX_LENGTH>>;

/// Structure used to updated the allowed sources list of a [Registration].
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct AllowedSourcesUpdate<A>
where
    A: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord + MaxEncodedLen,
{
    /// The update operation.
    pub operation: ListUpdateOperation,
    /// The [AccountId] to add or remove.
    pub account_id: A,
}

/// A Job ID consists of an [AccountId] and a [Script].
pub type JobId<AccountId> = (AccountId, Script);

/// Structure used to updated the certificate recovation list.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct CertificateRevocationListUpdate {
    /// The update operation.
    pub operation: ListUpdateOperation,
    /// The [AccountId] to add or remove.
    pub cert_serial_number: SerialNumber,
}

/// The allowed sources update operation.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
pub enum ListUpdateOperation {
    Add,
    Remove,
}

/// Structure representing a job registration.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq)]
pub struct JobRegistration<AccountId, Extra>
where
    AccountId: Parameter + Member + MaybeSerializeDeserialize + MaybeDisplay + Ord,
    Extra: Parameter + Member,
{
    /// The script to execute. It is a vector of bytes representing a utf8 string. The string needs to be a ipfs url that points to the script.
    pub script: Script,
    /// An optional array of the [AccountId]s allowed to fulfill the job. If the array is [None], then all sources are allowed.
    pub allowed_sources: Option<Vec<AccountId>>,
    /// A boolean indicating if only verified sources can fulfill the job. A verified source is one that has provided a valid key attestation.
    pub allow_only_verified_sources: bool,
    /// The schedule describing the desired (multiple) execution(s) of the script.
    pub schedule: Schedule,
    /// Maximum memory bytes used during a single execution of the job.
    pub memory: u32,
    /// Maximum network request used during a single execution of the job.
    pub network_requests: u32,
    /// Maximum storage bytes used during the whole period of the job's executions.
    pub storage: u32,
    /// Extra parameters. This type can be configured through [Config::RegistrationExtra].
    pub extra: Extra,
}

/// The desired schedule with some planning flexibility offered through `max_start_delay`.
///
/// ## Which planned schedules are valid?
///
/// Given `max_start_delay = 8`, `duration = 3`, `interval = 20`:
///
/// * planned delay is constant within the executions *of one slot*
///   ```ignore
///   SLOT 1: □□□□□□■■■□__________□□□□□□■■■□__________□□□□□□■■■□
///   SLOT 2: ■■■□□□□□□□__________■■■□□□□□□□__________■■■□□□□□□□
///   SLOT 3: □□■■■□□□□□__________□□■■■□□□□□__________□□■■■□□□□□
///   ```
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
pub struct Schedule {
    /// An upperbound for the duration of one execution of the script in milliseconds.
    pub duration: u64,
    /// Start time in milliseconds since Unix Epoch.
    pub start_time: u64,
    /// End time in milliseconds since Unix Epoch.
    ///
    /// Represents the end time (exclusive) in milliseconds since Unix Epoch
    /// of the period in which a job execution can start, independent of `duration` and `start_delay`.
    /// Hence all executions fit into `[start_time, end_time + duration + start_delay]`.
    pub end_time: u64,
    /// Interval at which to repeat execution in milliseconds.
    pub interval: u64,
    /// Maximum delay before each execution in milliseconds.
    pub max_start_delay: u64,
}

impl Schedule {
    /// The number of executions in the [`Schedule`] which corresponds to the length of [`Schedule::iter()`].
    pub fn execution_count(&self) -> u64 {
        (|| -> Option<u64> {
            self.end_time
                .checked_sub(self.start_time)?
                .checked_sub(1u64)?
                .checked_div(self.interval)?
                .checked_add(1u64)
        })()
        .unwrap_or(0u64)
    }

    /// Iterates over the start times of all the [`Schedule`]'s executions.
    ///
    /// All executions fit into `[start_time, end_time + duration + start_delay]`.
    /// Note that the last execution starts before `end_time` but may reach over it.
    /// This is so that *the number of executions does not depend on `start_delay`*.
    pub fn iter(&self, start_delay: u64) -> Option<ScheduleIter> {
        Some(ScheduleIter {
            delayed_start_time: self.start_time.checked_add(start_delay)?,
            delayed_end_time: self.end_time.checked_add(start_delay)?,
            interval: self.interval,
            current: None,
        })
    }
}

pub struct ScheduleIter {
    delayed_start_time: u64,
    delayed_end_time: u64,
    interval: u64,
    current: Option<u64>,
}

impl<'a> Iterator for ScheduleIter {
    type Item = u64;

    // Here, we define the sequence using `.current` and `.next`.
    // The return type is `Option<T>`:
    //     * When the `Iterator` is finished, `None` is returned.
    //     * Otherwise, the next value is wrapped in `Some` and returned.
    // We use Self::Item in the return type, so we can change
    // the type without having to update the function signatures.
    fn next(&mut self) -> Option<Self::Item> {
        self.current = match self.current {
            None => {
                if self.delayed_start_time < self.delayed_end_time {
                    Some(self.delayed_start_time)
                } else {
                    None
                }
            }
            Some(curr) => {
                let next = curr.checked_add(self.interval)?;
                if next < self.delayed_end_time {
                    Some(next)
                } else {
                    None
                }
            }
        };
        self.current
    }
}
