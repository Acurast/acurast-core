#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;
pub use traits::*;
pub use types::*;

mod traits;
mod types;

#[cfg(test)]
pub mod mock;
#[cfg(any(test, feature = "runtime-benchmarks"))]
mod stub;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
    use core::ops::Div;

    use codec::MaxEncodedLen;
    use frame_support::sp_runtime::Saturating;
    use frame_support::traits::tokens::Balance;
    use frame_support::{
        dispatch::DispatchResultWithPostInfo, log, pallet_prelude::*, traits::Get, Parameter,
    };
    use frame_system::{ensure_signed, pallet_prelude::OriginFor};
    use sp_arithmetic::traits::EnsureAddAssign;
    use sp_runtime::traits::{CheckedAdd, CheckedMul, CheckedSub};
    use sp_std::prelude::*;

    use crate::*;

    /// Configure the pallet by specifying the parameters and types on which it depends.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// Because this pallet emits events, it depends on the runtime's definition of an event.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        /// The the tolerance before a vester can be kicked out after his cooldown ended, as a time delta in milliseconds.
        ///
        /// A valid exit call that claims the full reward has to occur within `[cooldown end, now + DivestTolerance]`.
        /// Since the `now` timestmap is behind the current time up to the block time, the actual tolerance is sometimes higher than the configured.
        type DivestTolerance: Get<<Self as Config>::BlockNumber>;
        /// The maximum locking period in number of blocks. Vesting weights are linearly raised with [`Vesting`]`::locking_period / MaximumLockingPeriod`.
        #[pallet::constant]
        type MaximumLockingPeriod: Get<<Self as Config>::BlockNumber>;
        type Balance: Parameter + IsType<u128> + Div + Balance + MaybeSerializeDeserialize;
        #[pallet::constant]
        type BalanceUnit: Get<<Self as Config>::Balance>;
        type BlockNumber: Parameter
            + codec::Codec
            + MaxEncodedLen
            + Ord
            + CheckedAdd
            + Copy
            + Into<u128>
            + IsType<<Self as frame_system::Config>::BlockNumber>
            + MaybeSerializeDeserialize;
        type VestingBalance: VestingBalance<Self>;
        /// Weight Info for extrinsics.
        type WeightInfo: WeightInfo;
    }

    #[pallet::genesis_config]
    pub struct GenesisConfig<T: Config> {
        pub vesters: Vec<(T::AccountId, VestingFor<T>)>,
    }

    #[cfg(feature = "std")]
    impl<T: Config> Default for GenesisConfig<T> {
        fn default() -> Self {
            Self {
                vesters: Default::default(),
            }
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
        fn build(&self) {
            for (who, vesting) in &self.vesters {
                if let Err(e) = Pallet::<T>::vest_for(&who, vesting.to_owned()) {
                    log::error!(
                        target: "runtime::acurast_vesting",
                        "Vesting Genesis error: {:?}",
                        e,
                    );
                }
            }
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn pool)]
    pub(super) type Pool<T: Config> = StorageValue<_, PoolStateFor<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn vester_states)]
    pub(super) type VesterStates<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, VesterStateFor<T>>;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A vester started vesting. [vester_state, vesting]
        Vested(T::AccountId, VesterStateFor<T>),
        /// A vester revested. [vester, new_vester_state, during_cooldown]
        Revested(T::AccountId, VesterStateFor<T>, bool),
        /// A vester started cooldown. [vester, vester_state]
        CooldownStarted(T::AccountId, VesterStateFor<T>),
        /// A vester divests after his cooldown ended, claiming accrued rewards. [vester, vester_state_at_divest]
        Divested(T::AccountId, VesterStateFor<T>),
        /// A vester that exceeded his divest tolerance got kicked out. [vester, kicker, vester_state_at_divest, reward_cut]
        KickedOut(T::AccountId, T::AccountId, VesterStateFor<T>),
        /// A reward got distributed. [amount]
        RewardDistributed(T::Balance),
    }

    // Errors inform users that something went wrong.
    #[pallet::error]
    pub enum Error<T> {
        AlreadyVesting,
        MaximumLockingPeriodExceeded,
        NotVesting,
        CannotCooldownDuringCooldown,
        CannotRevestLess,
        CannotRevestWithShorterLockingPeriod,
        CannotDivestBeforeCooldownStarted,
        CannotDivestBeforeCooldownEnds,
        CannotDivestWhenToleranceEnded,
        CannotKickoutBeforeCooldown,
        CannotKickoutBeforeCooldownToleranceEnded,
        CalculationOverflow,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::vest())]
        pub fn vest(origin: OriginFor<T>, vesting: VestingFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let vester_state = Self::vest_for(&who, vesting)?;

            Self::deposit_event(Event::<T>::Vested(who, vester_state));

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::revest())]
        pub fn revest(origin: OriginFor<T>, vesting: VestingFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let (vester_state, cooldown_started_before) = <VesterStates<T>>::try_mutate(
                &who,
                |state| -> Result<(VesterStateFor<T>, bool), DispatchError> {
                    let state = state.as_mut().ok_or(Error::<T>::NotVesting)?;

                    if vesting.stake < state.stake {
                        Err(Error::<T>::CannotRevestLess)?
                    }
                    if vesting.locking_period < state.locking_period {
                        Err(Error::<T>::CannotRevestWithShorterLockingPeriod)?
                    }
                    if vesting.locking_period > <T as Config>::MaximumLockingPeriod::get() {
                        Err(Error::<T>::MaximumLockingPeriodExceeded)?
                    }

                    Self::accrue(state)?;

                    let cooldown_started_before = state.cooldown_started.is_some();

                    // recalculate the weight
                    let weight_before = state.weight;
                    let weight = Self::calculate_weight(&vesting)?;

                    state.locking_period = vesting.locking_period;
                    state.weight = weight;
                    state.stake = vesting.stake;
                    // record global s upper bound at time of revest
                    state.s = <Pool<T>>::get().s.1;
                    state.cooldown_started = None;

                    <Pool<T>>::try_mutate(|pool| -> Result<(), Error<T>> {
                        // due to rounding we need to substract the difference and not the new weight!
                        pool.total_weight.saturating_add(
                            // the new weight is always greater than the old weight, so check_sub should never fail
                            state
                                .weight
                                .checked_sub(&weight_before)
                                .ok_or(Error::<T>::CalculationOverflow)?,
                        );
                        Ok(())
                    })?;

                    Ok((state.clone(), cooldown_started_before))
                },
            )?;

            Self::deposit_event(Event::<T>::Revested(
                who,
                vester_state,
                cooldown_started_before,
            ));

            Ok(().into())
        }

        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::cooldown())]
        pub fn cooldown(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let vester_state = <VesterStates<T>>::try_mutate(
                &who,
                |state| -> Result<VesterStateFor<T>, DispatchError> {
                    let state = state.as_mut().ok_or(Error::<T>::NotVesting)?;

                    if let Some(_) = state.cooldown_started {
                        Err(Error::<T>::CannotCooldownDuringCooldown)?;
                    }

                    Self::accrue(state)?;

                    state.cooldown_started = Some(<frame_system::Pallet<T>>::block_number().into());

                    // punish divest with half the weight during cooldown
                    let weight_before = state.weight;
                    state.weight /= 2u128.into();

                    <Pool<T>>::try_mutate(|pool| -> Result<(), Error<T>> {
                        // due to rounding we need to substract the difference and not the new weight!
                        pool.total_weight
                            .checked_sub(
                                &weight_before
                                    .checked_sub(&state.weight)
                                    .ok_or(Error::<T>::CalculationOverflow)?,
                            )
                            .ok_or(Error::<T>::CalculationOverflow)?;
                        Ok(())
                    })?;

                    Ok(state.clone())
                },
            )?;

            Self::deposit_event(Event::<T>::CooldownStarted(who, vester_state));

            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::divest())]
        pub fn divest(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let vester_state = <VesterStates<T>>::try_mutate(
                &who,
                |state_| -> Result<VesterStateFor<T>, DispatchError> {
                    let state = state_.as_mut().ok_or(Error::<T>::NotVesting)?;

                    let cooldown_started = state
                        .cooldown_started
                        .ok_or(Error::<T>::CannotDivestBeforeCooldownStarted)?;

                    let current_block = <frame_system::Pallet<T>>::block_number();
                    if cooldown_started
                        .checked_add(&state.locking_period)
                        .ok_or(Error::<T>::CalculationOverflow)?
                        > current_block.into()
                    {
                        Err(Error::<T>::CannotDivestBeforeCooldownEnds)?
                    }

                    if cooldown_started
                        .checked_add(&state.locking_period)
                        .ok_or(Error::<T>::CalculationOverflow)?
                        .checked_add(&<T as Config>::DivestTolerance::get().into())
                        .ok_or(Error::<T>::CalculationOverflow)?
                        < current_block.into()
                    {
                        Err(Error::<T>::CannotDivestWhenToleranceEnded)?
                    }

                    Self::accrue(state)?;
                    let divest_state = *state;

                    *state_ = None;
                    Ok(divest_state)
                },
            )?;

            T::VestingBalance::pay_accrued(&who, vester_state.accrued)?;
            T::VestingBalance::unlock_stake(&who, vester_state.stake)?;

            Self::deposit_event(Event::<T>::Divested(who, vester_state));

            Ok(().into())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::kick_out())]
        pub fn kick_out(origin: OriginFor<T>, vester: T::AccountId) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let vester_state = <VesterStates<T>>::try_mutate(
                &vester,
                |state| -> Result<VesterStateFor<T>, DispatchError> {
                    let state = state.as_mut().ok_or(Error::<T>::NotVesting)?;

                    let cooldown_started = state
                        .cooldown_started
                        .ok_or(Error::<T>::CannotKickoutBeforeCooldown)?;

                    let current_block = <frame_system::Pallet<T>>::block_number();
                    if cooldown_started
                        .checked_add(&state.locking_period)
                        .ok_or(Error::<T>::CalculationOverflow)?
                        .checked_add(&<T as Config>::DivestTolerance::get().into())
                        .ok_or(Error::<T>::CalculationOverflow)?
                        >= current_block.into()
                    {
                        Err(Error::<T>::CannotKickoutBeforeCooldownToleranceEnded)?
                    }

                    Self::accrue(state)?;

                    Ok(*state)
                },
            )?;

            // give accrued to kicker (or part of it)
            T::VestingBalance::pay_kicker(&who, vester_state.accrued)?;
            T::VestingBalance::unlock_stake(&vester, vester_state.stake)?;

            Self::deposit_event(Event::<T>::KickedOut(vester, who, vester_state));

            Ok(().into())
        }
    }

    impl<T: Config> Pallet<T> {
        pub fn vest_for(
            who: &T::AccountId,
            vesting: VestingFor<T>,
        ) -> Result<VesterStateFor<T>, DispatchError> {
            // update vester state
            let vester_state = <VesterStates<T>>::try_mutate(
                &who,
                |state| -> Result<VesterStateFor<T>, DispatchError> {
                    if let Some(_) = state {
                        Err(Error::<T>::AlreadyVesting)?
                    }

                    if vesting.locking_period > <T as Config>::MaximumLockingPeriod::get() {
                        Err(Error::<T>::MaximumLockingPeriodExceeded)?
                    }

                    let weight = Self::calculate_weight(&vesting)?;

                    let s = VesterStateFor::<T> {
                        locking_period: vesting.locking_period,
                        weight: weight,
                        stake: vesting.stake,
                        accrued: 0u128.into(),
                        // record global s upper bound at time of vest
                        s: <Pool<T>>::get().s.1,
                        cooldown_started: None,
                    };
                    *state = Some(s);

                    // update global state
                    <Pool<T>>::try_mutate(|state| -> Result<(), DispatchError> {
                        // total_stake += stake
                        state
                            .total_stake
                            .ensure_add_assign(vesting.stake)
                            .map_err(|_| Error::<T>::CalculationOverflow)?;
                        // total_weight += weight
                        state
                            .total_weight
                            .ensure_add_assign(weight)
                            .map_err(|_| Error::<T>::CalculationOverflow)?;

                        Ok(())
                    })?;

                    Ok(s)
                },
            )?;

            T::VestingBalance::lock_stake(&who, vester_state.stake)?;
            Ok(vester_state.into())
        }

        pub fn distribute_reward(reward: T::Balance) -> DispatchResult {
            // s = s + reward / total_weight = s + reward * MaximumLockingPeriod / total_weight_numerator

            <Pool<T>>::try_mutate(|state| -> Result<(), DispatchError> {
                if state.total_weight > 0u128.into() {
                    state.s = (
                        state
                            .s
                            .0
                            .checked_add(
                                &(reward * <T as Config>::BalanceUnit::get() / state.total_weight),
                            )
                            .ok_or(Error::<T>::CalculationOverflow)?,
                        state
                            .s
                            .1
                            .checked_add(
                                &(reward
                                    // integer division, rounded up
                                    // (we already checked for state.total_weight > 0 to avoid DivisionByZero)
                                    .checked_add(&(state.total_weight - 1u128.into()))
                                    .ok_or(Error::<T>::CalculationOverflow)?
                                    * <T as Config>::BalanceUnit::get()
                                    / state.total_weight),
                            )
                            .ok_or(Error::<T>::CalculationOverflow)?,
                    );
                }

                Ok(())
            })?;

            Self::deposit_event(Event::<T>::RewardDistributed(reward));

            Ok(().into())
        }

        fn accrue(state: &mut VesterStateFor<T>) -> Result<(), Error<T>> {
            let pool = Self::pool();
            // reward = self.data.weight * (self.model.data.s - self.data.s)
            let reward = state
                .weight
                .checked_mul(
                    &pool
                        .s
                        // use minimal possible pool.s
                        .0
                        .checked_sub(&state.s)
                        .unwrap_or(0u128.into()),
                )
                .ok_or(Error::<T>::CalculationOverflow)?
                / <T as Config>::BalanceUnit::get();
            // accrued += reward
            state
                .accrued
                .ensure_add_assign(reward)
                .map_err(|_| Error::<T>::CalculationOverflow)?;
            // memorize maximum possible s
            state.s = pool.s.1;

            Ok(())
        }

        fn calculate_weight(vesting: &VestingFor<T>) -> Result<T::Balance, Error<T>> {
            let locking_period: u128 = vesting.locking_period.into();
            let max_locking_period: u128 = <T as Config>::MaximumLockingPeriod::get().into();
            // weight = locking_period / MaximumLockingPeriod * stake = locking_period * stake / MaximumLockingPeriod
            Ok((locking_period
                .checked_mul(vesting.stake.into())
                .ok_or(Error::<T>::CalculationOverflow)?
                / max_locking_period)
                .into())
        }
    }
}
