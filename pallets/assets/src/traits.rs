pub trait AssetValidator<AssetId> {
    type Error;

    fn validate(asset: &AssetId) -> Result<(), Self::Error>;
}
