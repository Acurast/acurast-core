use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec};
use sp_std::prelude::*;

use pallet_acurast::{JobId, JobModules, JobRegistration, MultiOrigin};

use crate::payments::RewardFor;
use crate::Config;

pub const MAX_PRICING_VARIANTS: u32 = 100;
pub const MAX_EXECUTIONS_PER_JOB: u64 = 10000;

pub const EXECUTION_OPERATION_HASH_MAX_LENGTH: u32 = 256;
pub const EXECUTION_FAILURE_MESSAGE_MAX_LENGTH: u32 = 1024;

pub type ExecutionOperationHash = BoundedVec<u8, ConstU32<EXECUTION_OPERATION_HASH_MAX_LENGTH>>;
pub type ExecutionFailureMessage = BoundedVec<u8, ConstU32<EXECUTION_FAILURE_MESSAGE_MAX_LENGTH>>;

pub type JobRegistrationForMarketplace<T> =
    JobRegistration<<T as frame_system::Config>::AccountId, <T as Config>::RegistrationExtra>;

/// Struct defining the extra fields for a `JobRegistration`.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, Eq)]
pub struct RegistrationExtra<Reward, Balance, AccountId>
where
    Reward: Parameter + Member,
{
    pub requirements: JobRequirements<Reward, AccountId>,
    pub expected_fulfillment_fee: Balance,
}

impl<Reward, Balance, AccountId> From<RegistrationExtra<Reward, Balance, AccountId>>
    for JobRequirements<Reward, AccountId>
where
    Reward: Parameter + Member,
{
    fn from(extra: RegistrationExtra<Reward, Balance, AccountId>) -> Self {
        extra.requirements
    }
}

/// The resource advertisement by a source containing pricing and capacity announcements.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct Advertisement<AccountId, AssetId, AssetAmount, MaxAllowedConsumers: Get<u32>> {
    /// The reward token accepted. Understood as one-of per job assigned.
    pub pricing: BoundedVec<PricingVariant<AssetId, AssetAmount>, ConstU32<MAX_PRICING_VARIANTS>>,
    /// Maximum memory in bytes not to be exceeded during any job's execution.
    pub max_memory: u32,
    /// Maximum network requests per second not to be exceeded.
    pub network_request_quota: u8,
    /// Storage capacity in bytes not to be exceeded in matching. The associated fee is listed in [pricing].
    pub storage_capacity: u32,
    /// An optional array of the [AccountId]s of consumers whose jobs should get accepted. If the array is [None], then jobs from all consumers are accepted.
    pub allowed_consumers: Option<BoundedVec<MultiOrigin<AccountId>, MaxAllowedConsumers>>,
    /// The modules available to the job on processor.
    pub available_modules: JobModules,
}

pub type AdvertisementFor<T> = Advertisement<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::AssetAmount,
    <T as Config>::MaxAllowedConsumers,
>;

/// The resource advertisement by a source containing the base restrictions.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct AdvertisementRestriction<AccountId> {
    /// Maximum memory in bytes not to be exceeded during any job's execution.
    pub max_memory: u32,
    /// Maximum network requests per second not to be exceeded.
    pub network_request_quota: u8,
    /// Storage capacity in bytes not to be exceeded in matching. The associated fee is listed in [pricing].
    pub storage_capacity: u32,
    /// An optional array of the [AccountId]s of consumers whose jobs should get accepted. If the array is [None], then jobs from all consumers are accepted.
    pub allowed_consumers: Option<Vec<MultiOrigin<AccountId>>>,
    /// The modules available to the job on processor.
    pub available_modules: JobModules,
}

/// Defines the scheduling window in which to accept matches for this pricing,
/// either as an absolute end time (in milliseconds since Unix Epoch)
/// or as a time delta (in milliseconds) added to the current time.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq, Copy)]
pub enum SchedulingWindow {
    /// Latest accepted end time of any matched job in milliseconds since Unix Epoch.
    End(u64),
    /// A time delta (in milliseconds) from now defining the window in which to accept jobs.
    ///
    /// Latest accepted end time of any matched job will be `now + delta`.
    Delta(u64),
}

/// Pricing variant listing cost per resource unit and slash on SLA violation.
/// Specified in specific asset that is payed out or deducted from stake on complete fulfillment.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Eq)]
pub struct PricingVariant<AssetId, AssetAmount> {
    /// The rewarded asset. Only one per [PricingVariant].
    pub reward_asset: AssetId,
    /// Fee per millisecond in [reward_asset].
    pub fee_per_millisecond: AssetAmount,
    /// Fee per storage byte in [reward_asset].
    pub fee_per_storage_byte: AssetAmount,
    /// A fixed base fee for each execution (for each slot and at each interval) in [reward_asset].
    pub base_fee_per_execution: AssetAmount,
    /// The scheduling window in which to accept matches for this pricing.
    pub scheduling_window: SchedulingWindow,
}

pub type PricingVariantFor<T> = PricingVariant<<T as Config>::AssetId, <T as Config>::AssetAmount>;

/// A proposed [Match] becomes an [Assignment] once it's acknowledged.
///
/// It's intended use is as part of a storage map that includes the job's and source's ID in its key.
///
/// The pricing agreed at the time of matching is stored along with an assignment.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct Assignment<Reward> {
    /// The 0-based slot index assigned to the source.
    pub slot: u8,
    /// The start delay for the first execution and all the following executions.
    pub start_delay: u64,
    /// The fee owed to source for each execution.
    pub fee_per_execution: Reward,
    /// If this assignment was acknowledged.
    pub acknowledged: bool,
    /// Keeps track of the SLA.
    pub sla: SLA,
}

pub type AssignmentFor<T> = Assignment<RewardFor<T>>;

/// The allowed sources update operation.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
pub enum JobStatus {
    /// Status after a job got registered.
    Open,
    /// Status after a valid match for a job got submitted.
    Matched,
    /// Status after a number of acknowledgments were submitted by sources.
    Assigned(u8),
    // The implicit final status leads to removal of job from status storage.
}

impl Default for JobStatus {
    fn default() -> Self {
        JobStatus::Open
    }
}

/// Keeps track of the SLA during and after a job's schedule is completed.
///
/// Also used to ensure that Acurast does not accept more than the expected number of reports (and pays out no more rewards).
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
pub struct SLA {
    pub total: u64,
    pub met: u64,
}

pub type JobRequirementsFor<T> =
    JobRequirements<RewardFor<T>, <T as frame_system::Config>::AccountId>;

/// Structure representing a job registration.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
pub struct JobRequirements<Reward, AccountId>
where
    Reward: Parameter + Member,
{
    /// The number of execution slots to be assigned to distinct sources. Either all or no slot get assigned by matching.
    pub slots: u8,
    /// Reward offered for each slot and scheduled execution of the job.
    pub reward: Reward,
    /// Minimum reputation required to process job, in parts per million, `r ∈ [0, 1_000_000]`.
    pub min_reputation: Option<u128>,
    /// Optional match provided with the job requirements. If provided, it gets processed instantaneously during
    /// registration call and validation errors lead to abortion of the call.
    pub instant_match: Option<Vec<PlannedExecution<AccountId>>>,
}

/// A (one-sided) matching of a job to sources such that the requirements of both sides, consumer and source, are met.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq)]
pub struct Match<AcurastAccountId> {
    /// The job to match.
    pub job_id: JobId<AcurastAccountId>,
    /// The sources to match each of the job's slots with.
    pub sources: Vec<PlannedExecution<AcurastAccountId>>,
}

/// The details for a single planned slot execution with the delay.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Eq, PartialEq)]
pub struct PlannedExecution<AccountId> {
    /// The source.
    pub source: AccountId,
    /// The start delay for the first execution and all the following executions.
    pub start_delay: u64,
}

#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub enum ExecutionResult {
    /// Success with operation hash.
    Success(ExecutionOperationHash),
    /// Failure with message.
    Failure(ExecutionFailureMessage),
}
