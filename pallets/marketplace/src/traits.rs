use frame_support::weights::Weight;

pub trait WeightInfo {
    fn advertise() -> Weight;
    fn delete_advertisement() -> Weight;
    // fn update_job_assignments() -> Weight;
}
