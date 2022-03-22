use cosmwasm_std::{OverflowError, StdError};
use cw20_base::ContractError as Cw20ContractError;
use cw721_base::ContractError as Cw721ContractError;
use std::convert::From;
use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("InvalidPrimitive")]
    InvalidPrimitive {},

    #[error("IllegalTokenName")]
    IllegalTokenName {},

    #[error("IllegalTokenSymbol")]
    IllegalTokenSymbol {},

    #[error("MinterBlacklisted")]
    MinterBlacklisted {},

    #[error("EmptyRecipientsList")]
    EmptyRecipientsList {},

    #[error("AmountExceededHundredPrecent")]
    AmountExceededHundredPrecent {},

    #[error("InvalidAddress")]
    InvalidAddress {},

    #[error("ExpirationInPast")]
    ExpirationInPast {},

    #[error("ExpirationNotSpecified")]
    ExpirationNotSpecified {},

    #[error("CannotOverwriteHeldFunds")]
    CannotOverwriteHeldFunds {},

    #[error("ContractAddressNotInAddressList")]
    ContractAddressNotInAddressList {},

    #[error("ModuleNotUnique")]
    ModuleNotUnique {},

    #[error("InvalidRate")]
    InvalidRate {},

    #[error("InsufficientFunds")]
    InsufficientFunds {},

    #[error("NoPendingPayments")]
    NoPendingPayments {},

    #[error("NoReceivingAddress")]
    NoReceivingAddress {},

    #[error("ModuleDiscriptionTooLong: {msg}")]
    ModuleDiscriptionTooLong { msg: String },

    #[error("SymbolInUse")]
    SymbolInUse {},

    #[error("ExceedsMaxAllowedCoins")]
    ExceedsMaxAllowedCoins {},

    #[error("NoLockedFunds")]
    NoLockedFunds {},

    #[error("FundsAreLocked")]
    FundsAreLocked {},

    #[error("InvalidTokenNameLength: {msg}")]
    InvalidTokenNameLength { msg: String },

    #[error("TokenIsArchived")]
    TokenIsArchived {},

    #[error("AuctionDoesNotExist")]
    AuctionDoesNotExist {},

    #[error("AuctionNotStarted")]
    AuctionNotStarted {},

    #[error("AuctionEnded")]
    AuctionEnded {},

    #[error("TokenOwnerCannotBid")]
    TokenOwnerCannotBid {},

    #[error("BidSmallerThanHighestBid")]
    BidSmallerThanHighestBid {},

    #[error("Overflow")]
    Overflow {},

    #[error("CannotWithdrawHighestBid")]
    CannotWithdrawHighestBid {},

    #[error("WithdrawalIsEmpty")]
    WithdrawalIsEmpty {},

    #[error("AuctionAlreadyStarted")]
    AuctionAlreadyStarted {},

    #[error("StartTimeAfterEndTime")]
    StartTimeAfterEndTime {},

    #[error("StartTimeInThePast")]
    StartTimeInThePast {},

    #[error("HighestBidderCannotOutBid")]
    HighestBidderCannotOutBid {},

    #[error("InvalidFunds: {msg}")]
    InvalidFunds { msg: String },

    #[error("AuctionRewardAlreadyClaimed")]
    AuctionAlreadyClaimed {},

    #[error("AuctionNotEnded")]
    AuctionNotEnded {},

    #[error("AuctionCancelled")]
    AuctionCancelled {},

    #[error("ExpirationMustNotBeNever")]
    ExpirationMustNotBeNever {},

    #[error("ExpirationsMustBeOfSameType")]
    ExpirationsMustBeOfSameType {},

    #[error("MoreThanOneCoin")]
    MoreThanOneCoin {},

    #[error("InvalidReplyId")]
    InvalidReplyId {},

    #[error("ParsingError: {err}")]
    ParsingError { err: String },

    #[error("MissingRequiredMessageData")]
    MissingRequiredMessageData {},

    #[error("Cannot migrate from different contract type: {previous_contract}")]
    CannotMigrate { previous_contract: String },

    #[error("NestedAndromedaMsg")]
    NestedAndromedaMsg {},

    #[error("UnexpectedExternalRate")]
    UnexpectedExternalRate {},

    #[error("DuplicateCoinDenoms")]
    DuplicateCoinDenoms {},

    // BEGIN CW20 ERRORS
    #[error("Cannot set to own account")]
    CannotSetOwnAccount {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("Allowance is expired")]
    Expired {},

    #[error("No allowance for this account")]
    NoAllowance {},

    #[error("Minting cannot exceed the cap")]
    CannotExceedCap {},

    #[error("Logo binary data exceeds 5KB limit")]
    LogoTooBig {},

    #[error("Invalid xml preamble for SVG")]
    InvalidXmlPreamble {},

    #[error("Invalid png header")]
    InvalidPngHeader {},
    // END CW20 ERRORS
    #[error("Invalid Module")]
    InvalidModule { msg: Option<String> },

    #[error("UnsupportedOperation")]
    UnsupportedOperation {},

    #[error("IncompatibleModules: {msg}")]
    IncompatibleModules { msg: String },

    #[error("ModuleDoesNotExist")]
    ModuleDoesNotExist {},

    #[error("token_id already claimed")]
    Claimed {},

    #[error("OfferAlreadyPlaced")]
    OfferAlreadyPlaced {},

    #[error("OfferLowerThanCurrent")]
    OfferLowerThanCurrent {},

    #[error("OfferNotExpired")]
    OfferNotExpired {},

    #[error("TransferAgreementExists")]
    TransferAgreementExists {},

    #[error("CannotDoubleWrapToken")]
    CannotDoubleWrapToken {},

    #[error("UnwrappingDisabled")]
    UnwrappingDisabled {},

    #[error("TokenNotWrappedByThisContract")]
    TokenNotWrappedByThisContract {},

    #[error("InvalidMetadata")]
    InvalidMetadata {},

    #[error("InvalidRecipientType: {msg}")]
    InvalidRecipientType { msg: String },

    #[error("InvalidTokensToWithdraw: {msg}")]
    InvalidTokensToWithdraw { msg: String },

    #[error("ModuleImmutable")]
    ModuleImmutable {},

    #[error("InvalidLtvRatio: {msg}")]
    InvalidLtvRatio { msg: String },

    #[error("No Ongoing Sale")]
    NoOngoingSale {},

    #[error("Token not for sale")]
    TokenNotForSale {},

    #[error("Token already purchased")]
    TokenAlreadyPurchased {},

    #[error("Purchase limit reached")]
    PurchaseLimitReached {},

    #[error("Sale not ended")]
    SaleNotEnded {},

    #[error("Min sales exceeded")]
    MinSalesExceeded {},

    #[error("Limit must not be zero")]
    LimitMustNotBeZero {},

    #[error("Sale has already started")]
    SaleStarted {},

    #[error("No purchases")]
    NoPurchases {},
}

impl From<Cw20ContractError> for ContractError {
    fn from(err: Cw20ContractError) -> Self {
        match err {
            Cw20ContractError::Std(std) => ContractError::Std(std),
            Cw20ContractError::Expired {} => ContractError::Expired {},
            Cw20ContractError::LogoTooBig {} => ContractError::LogoTooBig {},
            Cw20ContractError::NoAllowance {} => ContractError::NoAllowance {},
            Cw20ContractError::Unauthorized {} => ContractError::Unauthorized {},
            Cw20ContractError::CannotExceedCap {} => ContractError::CannotExceedCap {},
            Cw20ContractError::InvalidPngHeader {} => ContractError::InvalidPngHeader {},
            Cw20ContractError::InvalidZeroAmount {} => ContractError::InvalidZeroAmount {},
            Cw20ContractError::InvalidXmlPreamble {} => ContractError::InvalidXmlPreamble {},
            Cw20ContractError::CannotSetOwnAccount {} => ContractError::CannotSetOwnAccount {},
        }
    }
}

impl From<Cw721ContractError> for ContractError {
    fn from(err: Cw721ContractError) -> Self {
        match err {
            Cw721ContractError::Std(std) => ContractError::Std(std),
            Cw721ContractError::Expired {} => ContractError::Expired {},
            Cw721ContractError::Unauthorized {} => ContractError::Unauthorized {},
            Cw721ContractError::Claimed {} => ContractError::Claimed {},
        }
    }
}

impl From<FromUtf8Error> for ContractError {
    fn from(err: FromUtf8Error) -> Self {
        ContractError::Std(StdError::from(err))
    }
}

impl From<OverflowError> for ContractError {
    fn from(_err: OverflowError) -> Self {
        ContractError::Overflow {}
    }
}
