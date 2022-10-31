use pallet_acurast::JobRegistrationFor;

use crate::{AdvertisementFor, Config};

pub(crate) fn is_consumer_whitelisted<T: Config>(
    consumer: &T::AccountId,
    ad: &AdvertisementFor<T>,
) -> bool {
    ad.allowed_consumers
        .as_ref()
        .map(|allowed_consumers| {
            allowed_consumers
                .iter()
                .any(|allowed_consumer| allowed_consumer == consumer)
        })
        .unwrap_or(true)
}

pub fn is_source_whitelisted<T: Config>(
    source: &T::AccountId,
    registration: &JobRegistrationFor<T>,
) -> bool {
    registration
        .allowed_sources
        .as_ref()
        .map(|allowed_sources| {
            allowed_sources
                .iter()
                .any(|allowed_source| allowed_source == source)
        })
        .unwrap_or(true)
}
