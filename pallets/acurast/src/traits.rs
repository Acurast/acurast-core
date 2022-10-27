use crate::{Config, Fulfillment, JobRegistration, CertificateRevocationListUpdate, JobAssignmentUpdate};
use frame_support::{pallet_prelude::DispatchResultWithPostInfo, weights::Weight, PalletId};
use frame_system::pallet_prelude::OriginFor;
use sp_runtime::{traits::StaticLookup, Percent};
use sp_std::prelude::*;

/// This trait provides the interface for a fulfillment router.
pub trait FulfillmentRouter<T: Config> {
    fn received_fulfillment(
        origin: OriginFor<T>,
        from: T::AccountId,
        fulfillment: Fulfillment,
        registration: JobRegistration<T::AccountId, T::RegistrationExtra>,
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

// This trait provives methods for managing the fees.
pub trait FeeManager {
    fn get_fee_percentage() -> Percent;
    fn pallet_id() -> PalletId;
}
