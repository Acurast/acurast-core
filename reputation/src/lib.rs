#![cfg_attr(not(feature = "std"), no_std)]
pub mod reputation {
    /***
     * - each reputation update carries a weight ∈ [0, 1_000_000) depending on the size of the job reward
     * - reputation scores are discounted with a discounting factor λ
     * - Note that we apply integer arithmetic in the following calculations, due to floats not being allowed in the Substrate Runtime Storage
     * - Reputation scores have a precision of six decimal points
     * - Reputation scores are ∈ [0, 999_999]
     */

    const PRECISION: i64 = 1_000_000;
    const LAMBDA_N: i64 = 98; // lambda numerator
    const LAMBDA_D: i64 = 100; // lambda denominator

    pub trait ReputationEngine {
        fn update_reputation(
            beta_params: &mut BetaParams,
            fulfillment_successful: bool,
            job_reward: i64,
            avg_reward: i64,
        ) -> &BetaParams;
        fn get_reputation(r: i64, s: i64) -> i64;
        fn reputation(r: i64, s: i64) -> i64;
    }
    #[derive(Debug, Default, Clone, Copy)]
    pub struct BetaParams {
        pub r: i64,
        pub s: i64,
    }

    pub struct BetaReputation;
    impl ReputationEngine for BetaReputation {
        fn get_reputation(r: i64, s: i64) -> i64 {
            /***
             * In presence of discounting factor λ, the maximum reputation is given by (1/1-λ) / (1/1-λ) + 2
             * To scale possible reputation scores to [0,1), we thus have to account for this fact.
             */
            let reputation = Self::reputation(r, s);
            return reputation * ((LAMBDA_D) / (LAMBDA_D - LAMBDA_N) + 2)
                / ((LAMBDA_D) / (LAMBDA_D - LAMBDA_N));
        }

        fn reputation(r: i64, s: i64) -> i64 {
            return (r + 1) * PRECISION / ((r + s) + 2 * PRECISION);
        }

        fn update_reputation(
            mut beta_params: &mut BetaParams,
            fulfillment_successful: bool,
            job_reward: i64,
            avg_reward: i64,
        ) -> &BetaParams {
            fn weight(job_reward: i64, avg_reward: i64) -> i64 {
                return ((job_reward as f64 * PRECISION as f64)
                    / (job_reward as f64 + avg_reward as f64)) as i64;
            }
            let w = weight(job_reward, avg_reward);

            if fulfillment_successful {
                // a positive reputation update results in a reputation decrease if w < (r-λr+λs-s)/(s+1)
                let threshold = (beta_params.r - (LAMBDA_N * beta_params.r) / LAMBDA_D
                    + (LAMBDA_N * beta_params.s) / LAMBDA_D)
                    / (beta_params.s + 1);
                if w > (threshold) {
                    beta_params.s *= LAMBDA_N;
                    beta_params.s /= LAMBDA_D;
                    beta_params.r *= LAMBDA_N;
                    beta_params.r /= LAMBDA_D;

                    beta_params.r += w;
                }
            } else {
                beta_params.s *= LAMBDA_N;
                beta_params.s /= LAMBDA_D;
                beta_params.r *= LAMBDA_N;
                beta_params.r /= LAMBDA_D;

                beta_params.s += w;
            }
            return beta_params;
        }
    }

    mod tests {
        #[test]
        fn it_successfully_bootstraps() {
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};

            let mut beta_params = BetaParams::default();
            assert_eq!(beta_params.r, 0);
            assert_eq!(beta_params.s, 0);

            BetaReputation::update_reputation(&mut beta_params, true, 1, 0);
            assert_eq!(BetaReputation::get_reputation(beta_params.r, beta_params.s), 346_666);
        }

        #[test]
        fn it_calculates_the_lowest_score_as_zero() {
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};

            let job_reward = 108;
            let mut beta_params = BetaParams::default();

            for _i in 1..100 {
                BetaReputation::update_reputation(&mut beta_params, false, job_reward, job_reward);
            }
            assert_eq!(BetaReputation::get_reputation(beta_params.r, beta_params.s), 0);
        }
        #[test]
        fn it_has_reached_max_theoretical_reputation_after_600_fulfillments() {
            /***
             * in the theoretical case that each update has a weight of 1 (job_reward = 0)
             */
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};
            let job_reward = 108;

            let mut beta_params = BetaParams::default();

            for _i in 1..600 {
                BetaReputation::update_reputation(&mut beta_params, true, job_reward, 0);
            }
            assert_eq!(BetaReputation::get_reputation(beta_params.r, beta_params.s), 999_999);
        }

        #[test]
        fn it_has_reached_max_practical_reputation_after_600_fulfillments() {
            /***
             * in the practical case that each update has a weight of 0.5 (job_reward == avg_reward)
             */
            use crate::reputation::{BetaParams, BetaReputation, ReputationEngine};
            let job_reward = 108;

            let mut beta_params = BetaParams::default();

            for _i in 1..600 {
                BetaReputation::update_reputation(&mut beta_params, true, job_reward, job_reward);
                // avg_reward = 0 leads to weight = 1
            }
            assert_eq!(BetaReputation::get_reputation(beta_params.r, beta_params.s), 962_962);
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
                BetaReputation::update_reputation(&mut beta_params, true, job_reward, job_reward);
            }
            for _i in 1..50 {
                BetaReputation::update_reputation(&mut beta_params, false, job_reward, job_reward);
            }

            let reputation_i = BetaReputation::get_reputation(beta_params.r, beta_params.s);

            let mut beta_params = BetaParams::default();

            for _i in 1..75 {
                BetaReputation::update_reputation(&mut beta_params, true, job_reward, job_reward);
            }
            for _i in 1..50 {
                BetaReputation::update_reputation(&mut beta_params, false, job_reward, job_reward);
            }
            for _i in 1..25 {
                BetaReputation::update_reputation(&mut beta_params, true, job_reward, job_reward);
            }

            assert_eq!(BetaReputation::get_reputation(beta_params.r, beta_params.s), 567908);
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
                    BetaReputation::update_reputation(&mut beta_params, true, *reward, avg_reward);
                }

                assert_eq!(
                    BetaReputation::get_reputation(beta_params.r, beta_params.s),
                    expected_reputations[i]
                );
            }
        }

        #[test]
        fn it_may_decrease_reputation_after_positive_update() {
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
                BetaReputation::update_reputation(&mut beta_params, true, job_reward, job_reward);
            }
            let previous_rep = BetaReputation::get_reputation(beta_params.r, beta_params.s);
            BetaReputation::update_reputation(&mut beta_params, true, 1, job_reward);
            assert_eq!(BetaReputation::get_reputation(beta_params.r, beta_params.s), previous_rep);

            BetaReputation::update_reputation(&mut beta_params, false, 1, job_reward);
            assert!(BetaReputation::get_reputation(beta_params.r, beta_params.s) < previous_rep);
        }
    }
}
