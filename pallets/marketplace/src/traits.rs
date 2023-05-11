use frame_support::pallet_prelude::DispatchError;

/// Trait used to lookup the manager of a given processor account.
pub trait ManagerProvider<T: frame_system::Config> {
    fn manager_of(owner: &T::AccountId) -> Result<T::AccountId, DispatchError>;
}
