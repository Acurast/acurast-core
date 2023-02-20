#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use core::marker::PhantomData;
use num::{CheckedDiv, CheckedMul};
use scale_info::TypeInfo;
use sp_arithmetic::fixed_point::FixedU128;
use sp_arithmetic::traits::CheckedAdd;
use sp_arithmetic::{FixedPointNumber, FixedPointOperand, Permill};
use sp_core::RuntimeDebug;

#[cfg(test)]
mod tests;

const LAMBDA: FixedU128 = FixedU128::from_rational(98, 100);
const LAMBDA_F: FixedU128 = FixedU128::from_u32(1).div(FixedU128::from_u32(1).sub(LAMBDA));
/// In presence of discounting factor λ, the maximum reputation (excl.) is given by ((1/1-λ) + 1) / ((1/1-λ) + 2).
/// Using that maximum, we can scale reputation scores to [0,1).
const MAX_REPUTATION: FixedU128 = LAMBDA_F
    .add(FixedU128::from_u32(1))
    .div(LAMBDA_F.add(FixedU128::from_u32(2)));

pub trait ReputationEngine<T, P> {
    /// Calculates the normalized reputation.
    fn normalize(parameters: P) -> Option<Permill>;
    ///  Performs a reputation update and returns the adapated parameters.
    fn update(
        parameters: P,
        fulfillment_successful: bool,
        job_reward: T,
        avg_reward: T,
    ) -> Option<BetaParameters<FixedU128>>;
}

#[derive(
    RuntimeDebug, Encode, Decode, MaxEncodedLen, TypeInfo, Clone, Default, Copy, Eq, PartialEq,
)]
pub struct BetaParameters<T> {
    pub r: T,
    pub s: T,
}

pub struct BetaReputation<T: FixedPointOperand>(PhantomData<(T, BetaParameters<FixedU128>)>);

impl<T: FixedPointOperand> ReputationEngine<T, BetaParameters<FixedU128>> for BetaReputation<T> {
    /// Calculates the normalized reputation by `(r+1)/(r+s+2)`.
    fn normalize(params: BetaParameters<FixedU128>) -> Option<Permill> {
        params
            .r
            .checked_add(&1.into())?
            .checked_div(
                &params
                    .r
                    .checked_add(&params.s)?
                    .checked_add(&2.into())?,
            )?
            .checked_div(&MAX_REPUTATION)?
            .try_into_perthing()
            .ok()
    }

    ///  Performs a reputation update and returns the adapated parameters.
    ///  *  Each reputation update carries a `weight ∈ [0, 1]` depending on the size of the job reward
    ///  *  Reputation scores are discounted with a discounting factor `λ`
    ///  *  Reputation scores are `∈ [0, 1)`
    fn update(
        params: BetaParameters<FixedU128>,
        success: bool,
        job_reward: T,
        avg_reward: T,
    ) -> Option<BetaParameters<FixedU128>> {
        let w = weight(job_reward, FixedU128::saturating_from_integer(avg_reward))?;

        let r_ = match success {
            true => LAMBDA.checked_mul(&params.r)?.checked_add(&w)?,
            false => LAMBDA.checked_mul(&params.r)?,
        };
        let s_ = match success {
            true => LAMBDA.checked_mul(&params.s)?,
            false => LAMBDA.checked_mul(&params.s)?.checked_add(&w)?,
        };
        Some(BetaParameters { r: r_, s: s_ })
    }
}

/// Helper function calculating weight of an update.
fn weight<T: FixedPointOperand>(job_reward: T, avg_reward: FixedU128) -> Option<FixedU128> {
    let job_reward_ = FixedU128::saturating_from_integer(job_reward);
    job_reward_.checked_div(&avg_reward.checked_add(&job_reward_)?)
}
