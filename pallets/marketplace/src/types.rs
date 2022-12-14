use crate::payments::Reward as RewardTrait;
use acurast_common::BenchmarkDefault;
use frame_support::{pallet_prelude::*, storage::bounded_vec::BoundedVec};
use sp_std::prelude::*;
use xcm::prelude::*;

use pallet_acurast::JobRegistration;

use crate::Config;

pub const MAX_PRICING_VARIANTS: u32 = 100;

pub type JobRegistrationForMarketplace<T> =
    JobRegistration<<T as frame_system::Config>::AccountId, <T as Config>::RegistrationExtra>;

pub type AssetIdFor<T> = <<T as Config>::Reward as RewardTrait>::AssetId;
pub type AssetAmountFor<T> = <<T as Config>::Reward as RewardTrait>::AssetAmount;

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

pub type AdvertisementFor<T> =
    Advertisement<<T as frame_system::Config>::AccountId, AssetIdFor<T>, AssetAmountFor<T>>;

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

/// Structure representing a job registration.
#[derive(RuntimeDebug, Encode, Decode, TypeInfo, Clone, Eq, PartialEq)]
pub struct JobRequirements<T: crate::Config> {
    /// The number of execution slots to be assigned to distinct sources. Either all or no slot get assigned by matching.
    pub slots: u8,
    /// CPU milliseconds (upper bound) required to execute script.
    pub cpu_milliseconds: u128,
    /// Reward offered for the job
    pub reward: T::Reward,
}

pub type AcurastAssetId = u32;
pub type AcurastAssetAmount = u128;

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode, TypeInfo)]
pub struct AcurastAsset(pub MultiAsset);

impl crate::Reward for AcurastAsset {
    type AssetId = AcurastAssetId;
    type AssetAmount = AcurastAssetAmount;
    type Error = ();

    fn with_amount(&mut self, amount: Self::AssetAmount) -> Result<&Self, Self::Error> {
        self.0 = MultiAsset {
            id: self.0.id.clone(),
            fun: Fungible(amount),
        };
        Ok(self)
    }

    fn try_get_asset_id(&self) -> Result<Self::AssetId, Self::Error> {
        match &self.0.id {
            Concrete(location) => match location.last() {
                Some(GeneralIndex(id)) => (*id).try_into().map_err(|_| ()),
                _ => Err(()),
            },
            Abstract(_) => Err(()),
        }
    }

    fn try_get_amount(&self) -> Result<Self::AssetAmount, Self::Error> {
        match &self.0.fun {
            Fungible(amount) => Ok(*amount),
            _ => Err(()),
        }
    }
}

// used by benchmark tests
impl<T: Config<Reward = AcurastAsset>> BenchmarkDefault for JobRequirements<T> {
    fn benchmark_default() -> Self {
        let reward: T::Reward = (22u32, 1_000_000_000u128).into();
        JobRequirements {
            slots: 1,
            cpu_milliseconds: 5000,
            reward,
        }
    }
}

// by default acurast assets come from statemint
impl From<(u32, u128)> for AcurastAsset {
    fn from(tup: (u32, u128)) -> Self {
        AcurastAsset(MultiAsset {
            id: Concrete(MultiLocation {
                parents: 1,
                interior: X3(
                    Parachain(1000),
                    PalletInstance(50),
                    GeneralIndex(tup.0 as u128),
                ),
            }),
            fun: Fungible(tup.1),
        })
    }
}
