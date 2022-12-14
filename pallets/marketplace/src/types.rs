use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec};
use sp_std::prelude::*;

use pallet_acurast::JobRegistration;

use crate::payments::RewardFor;
use crate::Config;

pub const MAX_PRICING_VARIANTS: u32 = 100;

pub type JobRegistrationForMarketplace<T> =
    JobRegistration<<T as frame_system::Config>::AccountId, <T as Config>::RegistrationExtra>;

/// The resource advertisement by a source containing pricing and capacity announcements.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct Advertisement<AccountId, AssetId, AssetAmount> {
    /// The reward token accepted. Understood as one-of per job assigned.
    pub pricing: BoundedVec<PricingVariant<AssetId, AssetAmount>, ConstU32<MAX_PRICING_VARIANTS>>,
    // Capacity not to be exceeded in matching.
    pub capacity: u32,
    /// An optional array of the [AccountId]s of consumers whose jobs should get accepted. If the array is [None], then jobs from all consumers are accepted.
    pub allowed_consumers: Option<Vec<AccountId>>,
}

pub type AdvertisementFor<T> = Advertisement<
    <T as frame_system::Config>::AccountId,
    <T as Config>::AssetId,
    <T as Config>::AssetAmount,
>;

/// Pricing variant listing cost per resource unit and slash on SLA violation.
/// Specified in specific asset that is payed out or deducted from stake on complete fulfillment.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq)]
pub struct PricingVariant<AssetId, AssetAmount> {
    /// The rewarded asset. Only one per [PricingVariant].
    pub reward_asset: AssetId,
    /// Price in [reward_asset] per cpu second.
    pub price_per_cpu_millisecond: AssetAmount,
    /// A fixed bonus in [reward_asset].
    pub bonus: AssetAmount,
    /// The maximum slash to put at stake and that is lost if SLA is violated.
    pub maximum_slash: AssetAmount,
}

pub type AdvertisementIndexValue<AccountId, AssetAmount> = (AccountId, AssetAmount);

/// The allowed sources update operation.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
pub enum JobStatus {
    Open,
    Assigned,
    Fulfilled(SLAEvaluation),
}

impl Default for JobStatus {
    fn default() -> Self {
        JobStatus::Open
    }
}

/// Represents an evaluation of the SLA after a job's schedule is completed.
#[derive(RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, PartialEq, Copy)]
pub struct SLAEvaluation {
    pub total: u8,
    pub met: u8,
}

pub type JobRequirementsFor<T> = JobRequirements<RewardFor<T>>;

/// Structure representing a job registration.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
pub struct JobRequirements<Reward>
where
    Reward: Parameter + Member,
{
    /// The number of execution slots to be assigned to distinct sources. Either all or no slot get assigned by matching.
    pub slots: u8,
    /// CPU milliseconds (upper bound) required to execute script.
    pub cpu_milliseconds: u128,
    /// Reward offered for the job
    pub reward: Reward,
    /// Minimum reputation required to process job
    pub min_reputation: Option<u128>,
}

#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, PartialEq, Default)]
pub struct BetaParams<T: From<u128>> {
    pub r: T,
    pub s: T,
}
