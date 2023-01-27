use frame_support::{dispatch::Weight, pallet_prelude::DispatchResult, sp_runtime::DispatchError};

use crate::Config;

pub trait ManagerToken<T: Config> {
    fn create_token(id: T::ManagerId, owner: &T::AccountId) -> DispatchResult;
    fn manager_id_for_owner(owner: &T::AccountId) -> Result<T::ManagerId, DispatchError>;
}

pub trait WeightInfo {
    fn create_manager() -> Weight;
}
