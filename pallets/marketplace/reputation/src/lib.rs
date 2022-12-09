// #![cfg_attr(not(feature = "std"), no_std)]
pub mod reputation {
    use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub};
    use num::{CheckedDiv, CheckedMul};
    /***
     * - Each reputation update carries a weight ∈ [0, 1_000_000) depending on the size of the job reward
     * - Reputation scores are discounted with a discounting factor λ
     * - Note that we apply integer arithmetic in the following calculations, due to floats not being allowed in the Substrate Runtime Storage
     * - Reputation scores have a precision of six decimal points
     * - Reputation scores are ∈ [0, 999_999]
     */

    // TODO JGD introduce CheckedMul everywhere (also CheckedDiv?)
    const PRECISION: u128 = 1_000_000;
    const LAMBDA_N: u128 = 98; // lambda numerator
    const LAMBDA_D: u128 = 100; // lambda denominator

    pub trait ReputationEngine<T> {
        fn update_reputation(
            r: T,
            s: T,
            fulfillment_successful: bool,
            job_reward: T,
            avg_reward: T,
        ) -> BetaParams<T>;
        fn get_reputation(r: T, s: T) -> T;
        fn reputation(r: T, s: T) -> T;
        fn weight(job_reward: T, avg_reward: T) -> T;
    }
    #[derive(Debug, Default, Clone, Copy)]
    pub struct BetaParams<T> {
        pub r: T,
        pub s: T,
    }

    pub struct BetaReputation;
    impl<
            T: Div<Output = T>
                + Mul<Output = T>
                + CheckedMul
                + CheckedDiv
                + Sub<Output = T>
                + Add<Output = T>
                + Copy
                + PartialEq
                + PartialOrd
                + From<u128>
                + DivAssign
                + AddAssign
                + MulAssign
                + Into<u128>
                + Eq
                + Ord,
        > ReputationEngine<T> for BetaReputation
    {
        fn get_reputation(r: T, s: T) -> T {
            /***
             * In presence of discounting factor λ, the maximum reputation is given by (1/1-λ) / (1/1-λ) + 2
             * To scale possible reputation scores to [0,1), we thus have to account for this fact.
             */
            let n = T::from(LAMBDA_N);
            let d = T::from(LAMBDA_D);
            let reputation = Self::reputation(r, s);
            reputation.checked_mul(&n);
            return reputation
                .checked_mul(&((d) / (d - n) + T::from(2)))
                .unwrap()
                / ((d) / (d - n));
        }

        fn reputation(r: T, s: T) -> T {
            // (r+1)/(r+s+2)
            return (r + T::from(1))
                .checked_mul(&T::from(PRECISION))
                .unwrap()
                .checked_div(&((r + s) + T::from(2).checked_mul(&T::from(PRECISION)).unwrap()))
                .unwrap();
        }

        fn weight(job_reward: T, avg_reward: T) -> T {
            return (job_reward.checked_mul(&T::from(PRECISION)).unwrap())
                .checked_div(&(job_reward + avg_reward))
                .unwrap();
        }

        fn update_reputation(
            mut r: T,
            mut s: T,
            fulfillment_successful: bool,
            job_reward: T,
            avg_reward: T,
        ) -> BetaParams<T> {
            let w = Self::weight(job_reward, avg_reward);

            let n = T::from(LAMBDA_N);
            let d = T::from(LAMBDA_D);
            if fulfillment_successful {
                s *= n;
                s /= d;
                r *= n;
                r /= d;

                r += w;
            } else {
                s *= n;
                s /= d;
                r *= n;
                r /= d;

                s += w;
            }
            return BetaParams { r, s };
        }
    }

    mod tests {

        #[test]
        fn it_successfully_bootstraps() {
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};

            let mut beta_params = BetaParams::default();
            assert_eq!(beta_params.r, 0);
            assert_eq!(beta_params.s, 0);

            beta_params =
                BetaReputation::update_reputation(beta_params.r, beta_params.s, true, 1, 0);
            assert_eq!(
                BetaReputation::get_reputation(beta_params.r, beta_params.s),
                346_666
            );
        }

        #[test]
        fn it_calculates_the_lowest_score_as_zero() {
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};

            let job_reward = 108;
            let mut beta_params = BetaParams::default();

            for _i in 1..100 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    false,
                    job_reward,
                    job_reward,
                );
            }
            assert_eq!(
                BetaReputation::get_reputation(beta_params.r, beta_params.s),
                0
            );
        }
        #[test]
        fn it_has_reached_max_theoretical_reputation_after_600_consecutive_fulfillments() {
            /***
             * in the theoretical case that each update has a weight of 1 (job_reward = 0)
             */
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};
            let job_reward = 108;

            let mut beta_params = BetaParams::default();

            for _i in 1..600 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    true,
                    job_reward,
                    0, // avg_reward = 0 leads to weight = 1
                );
            }
            assert_eq!(
                BetaReputation::get_reputation(beta_params.r, beta_params.s),
                999_999
            );
        }

        #[test]
        fn it_has_reached_max_practical_reputation_after_600_consecutive_fulfillments() {
            /***
             * in the practical case that each update has a weight of 0.5 (job_reward == avg_reward)
             */
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};
            let job_reward = 108;

            let mut beta_params = BetaParams::default();

            for _i in 1..600 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    true,
                    job_reward,
                    job_reward,
                );
            }

            assert_eq!(
                BetaReputation::get_reputation(beta_params.r, beta_params.s),
                962_962
            );
        }

        #[test]
        fn it_discounts_older_reputation_updates() {
            /***
             * In both cases a processor has 100 successful and 50 unsuccessful fulfillments.
             * However the order in which they are incurred impacts the reputation score, with more recent scores having a bigger impact.
             */
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};

            let job_reward = 108;

            let mut beta_params = BetaParams::default();

            for _i in 1..100 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    true,
                    job_reward,
                    job_reward,
                );
            }
            for _i in 1..50 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    false,
                    job_reward,
                    job_reward,
                );
            }

            let reputation_i = BetaReputation::get_reputation(beta_params.r, beta_params.s);

            let mut beta_params = BetaParams::default();

            for _i in 1..75 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    true,
                    job_reward,
                    job_reward,
                );
            }
            for _i in 1..50 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    false,
                    job_reward,
                    job_reward,
                );
            }
            for _i in 1..25 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    true,
                    job_reward,
                    job_reward,
                );
            }

            assert_eq!(
                BetaReputation::get_reputation(beta_params.r, beta_params.s),
                567908
            );
            assert!(BetaReputation::get_reputation(beta_params.r, beta_params.s) > reputation_i);
        }

        #[test]
        fn it_updates_reputation_depending_on_size_of_job_reward() {
            /***
             * notice how the last entry of rewards_case_ii is greater than that of rewards_case_i,
             * leading to a higher weight of the respective reputation update and thus a higher reputation
             */
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};

            let rewards_case_i = [9, 8, 7, 6, 5, 4, 3, 2, 1];
            let rewards_case_ii = [9, 8, 7, 6, 5, 4, 3, 2, 11];

            let iterations = [rewards_case_i, rewards_case_ii];
            let expected_reputations = [641_350, 674_960];

            for (i, iteration) in iterations.iter().enumerate() {
                let mut beta_params = BetaParams::default();

                let mut total_jobs = 0;
                let mut total_rewards = 0;
                let mut avg_reward;

                for reward in iteration.iter() {
                    total_rewards += reward;
                    total_jobs += 1;
                    avg_reward = total_rewards / total_jobs;
                    beta_params = BetaReputation::update_reputation(
                        beta_params.r,
                        beta_params.s,
                        true,
                        *reward,
                        avg_reward,
                    );
                }

                assert_eq!(
                    BetaReputation::get_reputation(beta_params.r, beta_params.s),
                    expected_reputations[i]
                );
            }
        }

        #[test]
        fn it_never_decreases_reputation_after_positive_update() {
            /***
             * The combination of the weight and discounting parameter leads to an interesting behaviour:
             * A *positive* reputation update may lead to a *decrease* in reputation if the job reward and
             * thus the weight is sufficiently small.
             * Precisely, a positive reputation update results in a reputation decrease if w < (r-λr+λs-s)/(s+1)
             */
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};

            let job_reward = 108;

            let mut beta_params = BetaParams::default();

            for _i in 1..50 {
                beta_params = BetaReputation::update_reputation(
                    beta_params.r,
                    beta_params.s,
                    true,
                    job_reward,
                    job_reward,
                );
            }

            beta_params = BetaReputation::update_reputation(
                beta_params.r,
                beta_params.s,
                true,
                1,
                job_reward,
            );
            assert_eq!(
                BetaReputation::get_reputation(beta_params.r, beta_params.s),
                920_493
            );

            beta_params = BetaReputation::update_reputation(
                beta_params.r,
                beta_params.s,
                true,
                1,
                job_reward,
            );

            assert_eq!(
                BetaReputation::get_reputation(beta_params.r, beta_params.s),
                918_404
            );
        }
    }
}
