use crate::{
    AllowedSourcesUpdate, CertificateRevocationListUpdate, Config, Error, Fulfillment,
    JobAssignmentUpdate, JobRegistrationFor, Script,
};
use frame_support::{
    pallet_prelude::{DispatchResultWithPostInfo, Member},
    sp_runtime::{traits::StaticLookup, DispatchError},
    weights::Weight,
    Never, Parameter,
};
use frame_system::pallet_prelude::OriginFor;
use sp_std::prelude::*;

/// This trait provides the interface for a fulfillment router.
pub trait FulfillmentRouter<T: Config> {
    fn received_fulfillment(
        origin: OriginFor<T>,
        from: T::AccountId,
        fulfillment: Fulfillment,
        registration: JobRegistrationFor<T>,
        requester: <T::Lookup as StaticLookup>::Target,
    ) -> DispatchResultWithPostInfo;
}

pub trait RevocationListUpdateBarrier<T: Config> {
    fn can_update_revocation_list(
        origin: &T::AccountId,
        updates: &Vec<CertificateRevocationListUpdate>,
    ) -> bool;
}

impl<T: Config> RevocationListUpdateBarrier<T> for () {
    fn can_update_revocation_list(
        _origin: &T::AccountId,
        _updates: &Vec<CertificateRevocationListUpdate>,
    ) -> bool {
        false
    }
}

pub trait JobAssignmentUpdateBarrier<T: Config> {
    fn can_update_assigned_jobs(
        origin: &T::AccountId,
        updates: &Vec<JobAssignmentUpdate<T::AccountId>>,
    ) -> bool;
}

impl<T: Config> JobAssignmentUpdateBarrier<T> for () {
    fn can_update_assigned_jobs(
        _origin: &T::AccountId,
        _updates: &Vec<JobAssignmentUpdate<T::AccountId>>,
    ) -> bool {
        false
    }
}

pub trait WeightInfo {
    fn register() -> Weight;
    fn deregister() -> Weight;
    fn update_allowed_sources() -> Weight;
    fn update_job_assignments() -> Weight;
    fn fulfill() -> Weight;
    fn submit_attestation() -> Weight;
    fn update_certificate_revocation_list() -> Weight;
}

pub trait JobHooks<T: Config> {
    type Error: Into<Error<T>>;
    fn register_hook(
        who: &<T as frame_system::Config>::AccountId,
        registration: &JobRegistrationFor<T>,
    ) -> Result<(), DispatchError>;
    fn deregister_hook(
        who: &<T as frame_system::Config>::AccountId,
        script: &Script,
    ) -> Result<(), DispatchError>;
    fn update_allowed_sources_hook(
        who: &<T as frame_system::Config>::AccountId,
        script: &Script,
        updates: &Vec<AllowedSourcesUpdate<<T as frame_system::Config>::AccountId>>,
    ) -> Result<(), DispatchError>;
    fn fulfill_hook(
        who: &<T as frame_system::Config>::AccountId,
        fulfillment: &Fulfillment,
        requester: <T::Lookup as StaticLookup>::Target,
    ) -> Result<(), DispatchError>;
}

impl<T: Config> JobHooks<T> for () {
    type Error = ();
    fn register_hook(
        _who: &<T as frame_system::Config>::AccountId,
        _registration: &JobRegistrationFor<T>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
    fn deregister_hook(
        _who: &<T as frame_system::Config>::AccountId,
        _script: &Script,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
    fn update_allowed_sources_hook(
        _who: &<T as frame_system::Config>::AccountId,
        _script: &Script,
        _updates: &Vec<AllowedSourcesUpdate<<T as frame_system::Config>::AccountId>>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
    fn fulfill_hook(
        _who: &<T as frame_system::Config>::AccountId,
        _fulfillment: &Fulfillment,
        _requester: <T::Lookup as StaticLookup>::Target,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

impl<T: Config> From<()> for Error<T> {
    fn from(_: ()) -> Self {
        Self::JobHookFailed
    }
}

pub trait Reward {
    type AssetId;
    type Balance;
    type Error;

    fn with_amount(&mut self, amount: Self::Balance) -> Result<&Self, Self::Error>;
    fn try_get_asset_id(&self) -> Result<Self::AssetId, Self::Error>;
    fn try_get_amount(&self) -> Result<Self::Balance, Self::Error>;
}

impl Reward for () {
    type AssetId = Never;
    type Balance = Never;
    type Error = ();

    fn with_amount(&mut self, _: Self::Balance) -> Result<&Self, Self::Error> {
        Err(())
    }

    fn try_get_asset_id(&self) -> Result<Self::AssetId, Self::Error> {
        Err(())
    }

    fn try_get_amount(&self) -> Result<Self::Balance, Self::Error> {
        Err(())
    }
}

pub trait RewardManager<T: Config> {
    type Reward: Parameter + Member + Reward;

    fn lock_reward(
        reward: Self::Reward,
        owner: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError>;
    fn pay_reward(
        reward: Self::Reward,
        target: <T::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError>;
}

impl<T: Config> RewardManager<T> for () {
    type Reward = ();

    fn lock_reward(
        _reward: Self::Reward,
        _owner: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        Ok(())
    }

    fn pay_reward(
        _reward: Self::Reward,
        _target: <<T>::Lookup as StaticLookup>::Source,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}
