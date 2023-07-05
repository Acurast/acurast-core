use frame_support::weights::Weight;

pub trait WeightInfo {
    fn update_fee_percentage() -> Weight;
}

impl WeightInfo for () {
    fn update_fee_percentage() -> Weight {
        Weight::from_parts(10_000_000, 0)
    }
}
