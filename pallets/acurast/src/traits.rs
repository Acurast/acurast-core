use acurast_common::Attestation;
use frame_support::{
    pallet_prelude::DispatchResultWithPostInfo,
    sp_runtime::{traits::StaticLookup, DispatchError},
    weights::Weight,
};
use frame_system::pallet_prelude::OriginFor;
use sp_std::prelude::*;

use crate::{
    AllowedSourcesUpdate, CertificateRevocationListUpdate, Config, Error, Fulfillment,
    JobAssignmentUpdate, JobId, JobRegistrationFor, Script, StoredJobAssignment,
};

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

pub trait KeyAttestationBarrier<T: Config> {
    fn accept_attestation_for_origin(origin: &T::AccountId, attestation: &Attestation) -> bool;
}

impl<T: Config> KeyAttestationBarrier<T> for () {
    fn accept_attestation_for_origin(_origin: &<T>::AccountId, _attestation: &Attestation) -> bool {
        true
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
        registration: &JobRegistrationFor<T>,
    ) -> Result<(), DispatchError>;
}

impl<T: Config> JobHooks<T> for () {
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
        who: &<T as frame_system::Config>::AccountId,
        fulfillment: &Fulfillment,
        requester: <T::Lookup as StaticLookup>::Target,
        _registration: &JobRegistrationFor<T>,
    ) -> Result<(), DispatchError> {
        // find assignment
        let job_id: JobId<T::AccountId> = (requester.clone(), fulfillment.script.clone());
        let mut assigned_jobs = <StoredJobAssignment<T>>::get(&who).unwrap_or_default();
        let job_index = assigned_jobs
            .iter()
            .position(|assigned_job_id| assigned_job_id == &job_id)
            .ok_or(Error::<T>::FulfillSourceNotAllowed)?;

        // removed fulfilled job from assigned jobs
        assigned_jobs.remove(job_index);
        <StoredJobAssignment<T>>::set(&who, Some(assigned_jobs));

        Ok(())
    }
}

impl<T: Config> From<()> for Error<T> {
    fn from(_: ()) -> Self {
        Self::JobHookFailed
    }
}
