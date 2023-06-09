use acurast_common::{Attestation, JobId, MultiOrigin};
use frame_support::{sp_runtime::DispatchError, weights::Weight};
use sp_std::prelude::*;

use crate::{
    AllowedSourcesUpdate, CertificateRevocationListUpdate, Config, Error, JobRegistrationFor,
};

/// Allows to customize who can perform an update to the certificate revocation list.
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

/// Allows to customize the kind of key attestations that are accepted.
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
    fn submit_attestation() -> Weight;
    fn update_certificate_revocation_list() -> Weight;
}

/// Allows to hook additional logic for various job related extrinsics.
pub trait JobHooks<T: Config> {
    fn register_hook(
        who: &MultiOrigin<T::AccountId>,
        job_id: &JobId<<T as frame_system::Config>::AccountId>,
        registration: &JobRegistrationFor<T>,
    ) -> Result<(), DispatchError>;
    fn deregister_hook(
        who: &<T as frame_system::Config>::AccountId,
        job_id: &JobId<<T as frame_system::Config>::AccountId>,
    ) -> Result<(), DispatchError>;
    fn update_allowed_sources_hook(
        who: &<T as frame_system::Config>::AccountId,
        job_id: &JobId<<T as frame_system::Config>::AccountId>,
        updates: &Vec<AllowedSourcesUpdate<<T as frame_system::Config>::AccountId>>,
    ) -> Result<(), DispatchError>;
}

impl<T: Config> JobHooks<T> for () {
    fn register_hook(
        _who: &MultiOrigin<T::AccountId>,
        _job_id: &JobId<<T as frame_system::Config>::AccountId>,
        _registration: &JobRegistrationFor<T>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
    fn deregister_hook(
        _who: &<T as frame_system::Config>::AccountId,
        _job_id: &JobId<<T as frame_system::Config>::AccountId>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
    fn update_allowed_sources_hook(
        _who: &<T as frame_system::Config>::AccountId,
        _job_id: &JobId<<T as frame_system::Config>::AccountId>,
        _updates: &Vec<AllowedSourcesUpdate<<T as frame_system::Config>::AccountId>>,
    ) -> Result<(), DispatchError> {
        Ok(())
    }
}

impl<T: Config> From<()> for Error<T> {
    fn from(_: ()) -> Self {
        Self::JobHookFailed
    }
}
