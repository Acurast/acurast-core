use frame_support::dispatch::Weight;

pub trait WeightInfo {
    fn register() -> Weight;
    fn deregister() -> Weight;
    fn update_allowed_sources() -> Weight;
    fn advertise() -> Weight;
}

impl WeightInfo for () {
    fn register() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn deregister() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn update_allowed_sources() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn advertise() -> Weight {
        Weight::from_parts(10_000, 0)
    }
}
