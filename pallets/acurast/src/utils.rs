use crate::attestation::{
    extract_attestation, validate_certificate_chain, validate_certificate_chain_root,
};
use crate::{
    Attestation, AttestationChain, AttestationValidity, CertId, Config, Error, IssuerName,
    JobRegistration, SerialNumber, StoredAttestation, StoredRevokedCertificate, ValidatingCertIds,
};

pub(crate) fn validate_and_extract_attestation<T: Config>(
    attestation_chain: &AttestationChain,
) -> Result<Attestation, Error<T>> {
    validate_certificate_chain_root(&attestation_chain.certificate_chain)
        .map_err(|_| Error::<T>::RootCertificateValidationFailed)?;

    let (cert_ids, cert) = validate_certificate_chain(&attestation_chain.certificate_chain)
        .map_err(|_| Error::<T>::CertificateChainValidationFailed)?;

    let attestation_validity = AttestationValidity {
        not_before: cert.validity.not_before.timestamp_millis(),
        not_after: cert.validity.not_after.timestamp_millis(),
    };

    let key_description = extract_attestation(cert.extensions)
        .map_err(|_| Error::<T>::AttestationExtractionFailed)?;

    let cert_ids_bounded = cert_ids
        .into_iter()
        .map(|cert_id| {
            let (iss, sn) = cert_id;
            let iss_bounded = IssuerName::try_from(iss)
                .map_err(|_| Error::<T>::CannotGetAttestationIssuerName)?;
            let sn_bounded = SerialNumber::try_from(sn)
                .map_err(|_| Error::<T>::CannotGetAttestationSerialNumber)?;
            Ok((iss_bounded, sn_bounded))
        })
        .collect::<Result<Vec<CertId>, Error<T>>>()?;
    let cert_ids_bounded_vec = ValidatingCertIds::try_from(cert_ids_bounded)
        .map_err(|_| Error::<T>::CannotGetCertificateId)?;

    Ok(Attestation {
        cert_ids: cert_ids_bounded_vec,
        key_description: key_description
            .try_into()
            .map_err(|_| Error::<T>::AttestationToBoundedTypeConversionFailed)?,
        validity: attestation_validity,
    })
}

pub(crate) fn ensure_source_allowed<T: Config>(
    source: &T::AccountId,
    registration: &JobRegistration<T::AccountId, T::RegistrationExtra>,
) -> Result<(), Error<T>> {
    registration
        .allowed_sources
        .as_ref()
        .map(|allowed_sources| {
            allowed_sources
                .iter()
                .position(|allowed_source| allowed_source == source)
                .map(|_| ())
                .ok_or(Error::<T>::FulfillSourceNotAllowed)
        })
        .unwrap_or(Ok(()))?;

    if registration.allow_only_verified_sources {
        let attestation =
            <StoredAttestation<T>>::get(source).ok_or(Error::<T>::FulfillSourceNotVerified)?;
        ensure_not_expired(&attestation)?;
        ensure_not_revoked(&attestation)?;
    }

    Ok(())
}

pub(crate) fn ensure_not_expired<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
    let now: u64 = <pallet_timestamp::Pallet<T>>::now()
        .try_into()
        .map_err(|_| Error::<T>::FailedTimestampConversion)?;

    if now >= attestation.validity.not_after || now < attestation.validity.not_before {
        return Err(Error::<T>::AttestationCertificateNotValid);
    }
    let expire_date_time = (&attestation)
        .key_description
        .tee_enforced
        .usage_expire_date_time
        .or_else(|| {
            (&attestation)
                .key_description
                .software_enforced
                .usage_expire_date_time
        });
    if let Some(expire_date_time) = expire_date_time {
        if now >= expire_date_time {
            return Err(Error::<T>::AttestationUsageExpired);
        }
    }
    Ok(())
}

pub(crate) fn ensure_not_revoked<T: Config>(attestation: &Attestation) -> Result<(), Error<T>> {
    let ids = &attestation.cert_ids;
    for id in ids {
        if <StoredRevokedCertificate<T>>::get(&id.1).is_some() {
            return Err(Error::<T>::RevokedCertificate);
        }
    }
    Ok(())
}
