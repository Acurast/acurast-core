use sp_runtime::DispatchError;

pub trait AssetValidator<AssetId> {
    type Error: Into<DispatchError>;

    fn validate(asset: &AssetId) -> Result<(), Self::Error>;
}

impl<AssetId> AssetValidator<AssetId> for () {
    type Error = DispatchError;

    fn validate(_: &AssetId) -> Result<(), Self::Error> {
        Err(DispatchError::Other(""))
    }
}
