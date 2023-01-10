use acurast_common::Attestation;
use frame_support::{sp_runtime::DispatchError, weights::Weight};
use sp_std::prelude::*;

use crate::{
    AllowedSourcesUpdate, CertificateRevocationListUpdate, Config, Error, JobRegistrationFor,
    Script,
};

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
}

impl<T: Config> From<()> for Error<T> {
    fn from(_: ()) -> Self {
        Self::JobHookFailed
    }
}
