/// TODO: can be removed once we migrate to newer substrate version with XCMv3 and use [frame_support::traits::tokens::fungibles::Transfer] instead.
pub trait AssetTransfer {
    type AssetId;
    type Balance;
    type AccountId;
    type Error;

    fn transfer(
        asset: Self::AssetId,
        from: &Self::AccountId,
        to: &Self::AccountId,
        amount: Self::Balance,
    ) -> Result<(), Self::Error>;
}
