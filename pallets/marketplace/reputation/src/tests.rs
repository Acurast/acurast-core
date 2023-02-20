#![cfg(test)]

use crate::{BetaParameters, BetaReputation, ReputationEngine};
use sp_arithmetic::fixed_point::FixedU128;
use sp_arithmetic::Permill;

#[test]
fn neutral_reputation() {
    let mut beta_params: BetaParameters<FixedU128> = BetaParameters::default();
    assert_eq!(beta_params.r, FixedU128::from_u32(0));
    assert_eq!(beta_params.s, FixedU128::from_u32(0));

    assert_eq!(
        BetaReputation::<u128>::normalize(beta_params),
        Some(Permill::from_rational(509803u32, 1_000_000))
    );
}

#[test]
fn one_success() {
    let mut beta_params: BetaParameters<FixedU128> = BetaParameters::default();
    assert_eq!(beta_params.r, FixedU128::from_u32(0));
    assert_eq!(beta_params.s, FixedU128::from_u32(0));

    beta_params = BetaReputation::update(beta_params, true, 1, 0).unwrap();

    assert_eq!(
        BetaReputation::<u128>::normalize(beta_params),
        Some(Permill::from_rational(679738u32, 1_000_000))
    );
}

#[test]
fn calculates_the_lowest_score_as_zero() {
    let job_reward = 108;
    let mut beta_params = BetaParameters::default();

    for _i in 1..100 {
        beta_params = BetaReputation::update(beta_params, false, job_reward, job_reward).unwrap();
    }
    assert_eq!(
        BetaReputation::<u128>::normalize(beta_params),
        Some(Permill::from_rational(43172u32, 1_000_000))
    );
}

#[test]
fn has_reached_max_theoretical_reputation_after_600_consecutive_fulfillments() {
    /***
     * in the theoretical case that each update has a weight of 1 (job_reward = 0)
     */
    use crate::{BetaParameters, BetaReputation, ReputationEngine};
    let job_reward = 108;

    let mut beta_params = BetaParameters::default();

    for _i in 1..600 {
        beta_params = BetaReputation::update(
            beta_params,
            true,
            job_reward,
            0, // avg_reward = 0 leads to weight = 1
        )
        .unwrap();
    }

    assert_eq!(
        BetaReputation::<u128>::normalize(beta_params),
        Some(Permill::from_parts(999_999))
    );
}

#[test]
fn has_reached_max_practical_reputation_after_600_consecutive_fulfillments() {
    /***
     * in the practical case that each update has a weight of 0.5 (job_reward == avg_reward)
     */
    let job_reward = 108;

    let mut beta_params = BetaParameters::default();

    for _i in 1..60 {
        beta_params = BetaReputation::update(beta_params, true, job_reward, job_reward).unwrap();
    }

    assert_eq!(
        BetaReputation::<u128>::normalize(beta_params),
        Some(Permill::from_rational(967076u32, 1_000_000))
    );
}

#[test]
fn discounts_older_reputation_updates() {
    /***
     * In both cases a processor has 100 successful and 50 unsuccessful fulfillments.
     * However the order in which they are incurred impacts the reputation score, with more recent scores having a bigger impact.
     */

    let job_reward = 108;

    let mut beta_params = BetaParameters::default();

    for _i in 1..100 {
        beta_params = BetaReputation::update(beta_params, true, job_reward, job_reward).unwrap();
    }
    for _i in 1..50 {
        beta_params = BetaReputation::update(beta_params, false, job_reward, job_reward).unwrap();
    }

    let reputation_i = BetaReputation::<u128>::normalize(beta_params);

    let mut beta_params = BetaParameters::default();

    for _i in 1..75 {
        beta_params = BetaReputation::update(beta_params, true, job_reward, job_reward).unwrap();
    }
    for _i in 1..50 {
        beta_params = BetaReputation::update(beta_params, false, job_reward, job_reward).unwrap();
    }
    for _i in 1..25 {
        beta_params = BetaReputation::update(beta_params, true, job_reward, job_reward).unwrap();
    }

    assert_eq!(
        BetaReputation::<u128>::normalize(beta_params),
        Some(Permill::from_rational(596420u32, 1_000_000))
    );
    assert!(BetaReputation::<u128>::normalize(beta_params) > reputation_i);
}

#[test]
fn updates_reputation_depending_on_size_of_job_reward() {
    /***
     * notice how the last entry of rewards_case_ii is greater than that of rewards_case_i,
     * leading to a higher weight of the respective reputation update and thus a higher reputation
     */

    let rewards_case_i = [9, 8, 7, 6, 5, 4, 3, 2, 1];
    let rewards_case_ii = [9, 8, 7, 6, 5, 4, 3, 2, 11];

    let iterations = [rewards_case_i, rewards_case_ii];
    let expected_reputations = [
        Permill::from_rational(824191u32, 1_000_000),
        Permill::from_rational(840667u32, 1_000_000),
    ];

    for (i, iteration) in iterations.iter().enumerate() {
        let mut beta_params = BetaParameters::default();

        let mut total_jobs = 0;
        let mut total_rewards = 0;
        let mut avg_reward;

        for reward in iteration.iter() {
            total_rewards += reward;
            total_jobs += 1;
            avg_reward = total_rewards / total_jobs;
            beta_params = BetaReputation::update(beta_params, true, *reward, avg_reward).unwrap();
        }

        assert_eq!(
            BetaReputation::<u128>::normalize(beta_params),
            Some(expected_reputations[i])
        );
    }
}

#[test]
fn never_decreases_reputation_after_positive_update_for_average_reward() {
    /***
     * The combination of the weight and discounting parameter leads to an interesting behaviour:
     * A *positive* reputation update may lead to a *decrease* in reputation if the job reward and
     * thus the weight is sufficiently small.
     * Precisely, a positive reputation update results in a reputation decrease if w < (r-λr+λs-s)/(s+1)
     */
    let job_reward = 108;

    let mut beta_params = BetaParameters::default();
    let mut reputation = Permill::zero();
    for _i in 1..50 {
        beta_params = BetaReputation::update(beta_params, true, job_reward, job_reward).unwrap();

        let new_reputation = BetaReputation::<u128>::normalize(beta_params).unwrap();
        assert!(reputation < new_reputation);
        reputation = new_reputation;
    }
}
