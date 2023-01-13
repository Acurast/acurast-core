#![cfg(test)]

use crate::Schedule;
macro_rules! tests {
    ($property_test_func:ident {
        $( $(#[$attr:meta])* $test_name:ident( $( $param:expr ),* ); )+
    }) => {
        $(
            $(#[$attr])*
            #[test]
            fn $test_name() {
                $property_test_func($( $param ),* )
            }
        )+
    }
}

fn test_schedule_execution_count(schedule: Schedule, exp_count: u64) {
    assert_eq!(schedule.execution_count(), exp_count);
}

tests! {
    test_schedule_execution_count {
        // ╭start ╭end
        // ■■■■■■■■
        test_schedule_execution_count_tight(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 8,
                interval: 2,
                max_start_delay: 0,
            },
            4
        );
        // ╭start         ╭end
        // ■■___■■___■■___
        test_schedule_execution_count_fit(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 15,
                interval: 5,
                max_start_delay: 0,
            },
            3
        );
        // ╭start   ╭end
        // ■■___■■__
        test_schedule_execution_count_last_not_fitting(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 9,
                interval: 5,
                max_start_delay: 0,
            },
            2
        );
        // ╭start         ╭end
        // ■■□□_■■□□_■■□□_
        // □■■□_□■■□_□■■□_
        // □□■■_□□■■_□□■■_
        test_schedule_execution_count_delay_fit(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 15,
                interval: 5,
                max_start_delay: 2,
            },
            3
        );
        // ╭start   ╭end
        // ■■□□_■■□□
        // □■■□_□■■□
        // □□■■_□□■■
        test_schedule_execution_count_delay_last_not_fitting(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 9,
                interval: 5,
                max_start_delay: 2,
            },
            2
        );
        // ╭start     ╭end
        // ■■□□_■■□□_■■□□
        // □■■□_□■■□_□■■□
        // □□■■_□□■■_□□■■
        test_schedule_execution_count_delay_reaching_over_end(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 11,
                interval: 5,
                max_start_delay: 2,
            },
            3
        );
        // ╭start=end
        //
        test_schedule_execution_count_zero_fits(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 0,
                interval: 5,
                max_start_delay: 2,
            },
            0
        );
        // ╭start=end-1
        // ■■□□
        // □■■□
        // □□■■
        test_schedule_execution_count_one_fit(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 1,
                interval: 5,
                max_start_delay: 2,
            },
            1
        );
        //    ╭start     ╭end
        // ___■■□□_■■□□_■■□□
        // ___□■■□_□■■□_□■■□
        // ___□□■■_□□■■_□□■■
        test_schedule_execution_count_start_time_indifferent(
            Schedule{
                duration: 2,
                start_time: 3,
                end_time: 14,
                interval: 5,
                max_start_delay: 2,
            },
            3
        );
    }
}

fn test_schedule_iter(schedule: Schedule, start_delay: u64, exp_starts: Vec<u64>) {
    assert_eq!(
        schedule.iter(start_delay).unwrap().collect::<Vec<u64>>(),
        exp_starts
    );
}

tests! {
    test_schedule_iter {
        // ╭start  ╭end
        // ■■■■■■■■
        test_schedule_iter_tight(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 8,
                interval: 2,
                max_start_delay: 0,
            },
            0,
            vec![0,2,4,6]
        );
        // ╭start         ╭end
        // ■■___■■___■■___
        test_schedule_iter_fit(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 15,
                interval: 5,
                max_start_delay: 0,
            },
            0,
            vec![0,5,10]
        );
        // ╭start   ╭end
        // ■■___■■__
        test_schedule_iter_last_not_fitting(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 9,
                interval: 5,
                max_start_delay: 0,
            },
            0,
            vec![0,5]
        );
        // ╭start         ╭end
        // ■■□□_■■□□_■■□□_
        // □■■□_□■■□_□■■□_
        // □□■■_□□■■_□□■■_
        test_schedule_iter_delay_fit(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 15,
                interval: 5,
                max_start_delay: 2,
            },
            0,
            vec![0,5,10]
        );
        // ╭start   ╭end
        // ■■□□_■■□□
        // □■■□_□■■□
        // □□■■_□□■■
        test_schedule_iter_delay_last_not_fitting(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 9,
                interval: 5,
                max_start_delay: 2,
            },
            0,
            vec![0,5]
        );
        // ╭start     ╭end
        // ■■□□_■■□□_■■□□
        // □■■□_□■■□_□■■□
        // □□■■_□□■■_□□■■
        test_schedule_iter_delay_reaching_over_end(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 11,
                interval: 5,
                max_start_delay: 2,
            },
            0,
            vec![0,5,10]
        );
        // ╭start=end
        //
        test_schedule_iter_zero_fits(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 0,
                interval: 5,
                max_start_delay: 2,
            },
            0,
            vec![]
        );
        // ╭start=end-1
        // ■■□□
        // □■■□
        // □□■■
        test_schedule_iter_one_fit(
            Schedule{
                duration: 2,
                start_time: 0,
                end_time: 1,
                interval: 5,
                max_start_delay: 2,
            },
            0,
            vec![0]
        );
    }
}
