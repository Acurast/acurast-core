use frame_support::dispatch::Weight;

pub trait WeightInfo {
    fn register() -> Weight;
    fn deregister() -> Weight;
    fn update_allowed_sources() -> Weight;
    fn advertise() -> Weight;
}

impl WeightInfo for () {
    fn register() -> Weight {
        Weight::from_ref_time(10_000)
    }

    fn deregister() -> Weight {
        Weight::from_ref_time(10_000)
    }

    fn update_allowed_sources() -> Weight {
        Weight::from_ref_time(10_000)
    }

    fn advertise() -> Weight {
        Weight::from_ref_time(10_000)
    }
}
