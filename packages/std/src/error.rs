use cosmwasm_std::{Addr, OverflowError, StdError};
use cw20_base::ContractError as Cw20ContractError;
use cw721_base::ContractError as Cw721ContractError;
use cw_asset::AssetError;
pub use cw_utils::{ParseReplyError, PaymentError, ThresholdError};
use hex::FromHexError;
use std::convert::From;
use std::str::{ParseBoolError, Utf8Error};
use std::string::FromUtf8Error;
use thiserror::Error;

use crate::common::Milliseconds;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Hex(#[from] FromHexError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("{0}")]
    ParseReplyError(#[from] ParseReplyError),

    #[error("{0}")]
    Threshold(#[from] ThresholdError),

    #[error("Unauthorized")]
    Unauthorized {},

    /// Used by external developers in case they don't find their desired error message
    #[error("CustomError: {msg}")]
    CustomError { msg: String },

    #[error("ActionNotFound")]
    ActionNotFound {},
    #[error("UnpublishedCodeID")]
    UnpublishedCodeID {},

    #[error("UnpublishedVersion")]
    UnpublishedVersion {},

    #[error("ContractLocked: {msg:?}")]
    ContractLocked { msg: Option<String> },

    #[error("UnidentifiedMsgID")]
    UnidentifiedMsgID {},

    #[error("UnmetCondition")]
    UnmetCondition {},

    #[error("InvalidOrigin")]
    InvalidOrigin {},

    #[error("EmptyDenom")]
    EmptyDenom {},

    #[error("NoDenomInfoProvided")]
    NoDenomInfoProvided {},

    #[error("Cannot assign cw20 rate to cross-chain recipient")]
    InvalidCw20CrossChainRate {},

    #[error("InvalidAmount: {msg}")]
    InvalidAmount { msg: String },

    #[error("OverlappingRanges")]
    OverlappingRanges {},

    #[error("Invalid {operation} Operation with {validator}")]
    InvalidValidatorOperation {
        operation: String,
        validator: String,
    },

    #[error("Invalid Campaign Operation: {operation} on {stage}")]
    InvalidCampaignOperation { operation: String, stage: String },

    #[error("No Staking Reward")]
    InvalidClaim {},

    #[error("InvalidSender")]
    InvalidSender {},

    #[error("InvalidSigner")]
    InvalidSigner { signer: String },

    #[error("InvalidValidator")]
    InvalidValidator {},

    #[error("NoActorsProvided")]
    NoActorsProvided {},

    #[error("InvalidDelegation")]
    InvalidDelegation {},

    #[error("Invalid action: {action}. Expected either SEND_NFT or SEND_CW20")]
    InvalidAction { action: String },

    #[error("RewardTooLow")]
    RewardTooLow {},

    #[error("InvalidRedelegationAmount. Got {{amount}}, full delegation: {{max}}")]
    InvalidRedelegationAmount { amount: String, max: String },

    #[error("LockedNFT")]
    LockedNFT {},

    #[error("UserNotFound")]
    UserNotFound {},

    #[error("ProcessNotFound")]
    ProcessNotFound {},

    #[error("only unordered channels are supported")]
    OrderedChannel {},

    #[error("Invalid Expiration Time")]
    InvalidExpirationTime {},

    #[error("invalid IBC channel version - got ({actual}), expected ({expected})")]
    InvalidVersion { actual: String, expected: String },

    #[error("CrossChainComponentsCurrentlyDisabled")]
    CrossChainComponentsCurrentlyDisabled {},

    #[error("tokenId list has different length than tokenUri list")]
    TokenInfoLenMissmatch {},

    #[error("ICS 721 channels may not be closed")]
    CantCloseChannel {},

    #[error("Paused")]
    Paused {},

    #[error("EmptyOptional")]
    EmptyOptional {},

    #[error("EmptyEvents")]
    EmptyEvents {},

    #[error("EmptyUnstakingQueue")]
    EmptyUnstakingQueue {},

    #[error("EmptyString")]
    EmptyString {},

    #[error("EmptyClassId")]
    EmptyClassId {},

    #[error("NoTokens")]
    NoTokens {},

    #[error("NoBuyNowOption")]
    NoBuyNowOption {},

    #[error("UnrecognisedReplyId")]
    UnrecognisedReplyId {},

    #[error("ImbalancedTokenInfo")]
    ImbalancedTokenInfo {},

    #[error("UnsupportedNFT")]
    UnsupportedNFT {},

    #[error("UnsupportedReturnType")]
    UnsupportedReturnType {},

    #[error("UnsupportedProtocol")]
    UnsupportedProtocol {},

    #[error("AlreadyUnbonded")]
    AlreadyUnbonded {},

    #[error("NFTNotFound")]
    NFTNotFound {},

    #[error("PriceNotSet")]
    PriceNotSet {},

    #[error("InvalidPrimitive")]
    InvalidPrimitive {},

    #[error("InvalidPublickey")]
    InvalidPublickey {},

    #[error("StillBonded")]
    StillBonded {},

    #[error("ParseBoolError")]
    ParseBoolError {},

    #[error("NoResponseElementNeeded")]
    NoResponseElementNeeded {},

    #[error("ResponseElementRequired")]
    ResponseElementRequired {},

    #[error("InsufficientBondedTime")]
    InsufficientBondedTime {},

    #[error("ThresholdsPercentagesDiscrepancy: {msg}")]
    ThresholdsPercentagesDiscrepancy { msg: String },

    #[error("LockTimeTooShort")]
    LockTimeTooShort {},

    #[error("LockTimeTooLong")]
    LockTimeTooLong {},

    #[error("InvalidWeight")]
    InvalidWeight {},

    #[error("NoResults")]
    NoResults {},

    #[error("NotEnoughTokens")]
    NotEnoughTokens {},

    #[error("MissingParameters")]
    MissingParameters {},

    #[error("OnlyOneSourceAllowed")]
    OnlyOneSourceAllowed {},

    #[error("IllegalTokenName")]
    IllegalTokenName {},

    #[error("IllegalTokenSymbol")]
    IllegalTokenSymbol {},

    #[error("Refilling")]
    Refilling {},

    #[error("WithdrawalLimitExceeded")]
    WithdrawalLimitExceeded {},

    #[error("CoinNotFound")]
    CoinNotFound {},

    #[error("NotInRefillMode")]
    NotInRefillMode {},

    #[error("NotEnoughResults")]
    NotEnoughResults {},

    #[error("ReachedRecipientLimit")]
    ReachedRecipientLimit {},

    #[error("MinterBlacklisted")]
    MinterBlacklisted {},

    #[error("EmptyRecipientsList")]
    EmptyRecipientsList {},

    #[error("EmptyThresholdsList")]
    EmptyThresholdsList {},

    #[error("AmountExceededHundredPrecent")]
    AmountExceededHundredPrecent {},

    #[error("InvalidAddress")]
    InvalidAddress {},

    #[error("ExpirationInPast")]
    ExpirationInPast {},

    #[error("ExecuteError")]
    ExecuteError {},

    #[error("UnspecifiedWithdrawalFrequency")]
    UnspecifiedWithdrawalFrequency {},

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

    #[error("TemporarilyDisabled")]
    TemporarilyDisabled {},

    #[error("AccountNotFound")]
    AccountNotFound {},

    #[error("ActorNotFound")]
    ActorNotFound {},

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

    #[error("SaleDoesNotExist")]
    SaleDoesNotExist {},

    #[error("AuctionNotStarted")]
    AuctionNotStarted {},

    #[error("AuctionEnded")]
    AuctionEnded {},

    #[error("AuctionBought")]
    AuctionBought {},

    #[error("CampaignNotStarted")]
    CampaignNotStarted {},

    #[error("CampaignEnded")]
    CampaignEnded {},

    #[error("Campaign is not expired yet")]
    CampaignNotExpired {},

    #[error("SaleNotStarted")]
    SaleNotStarted {},

    #[error("SaleEnded")]
    SaleEnded {},

    #[error("SaleNotOpen")]
    SaleNotOpen {},

    #[error("SaleExpired")]
    SaleExpired {},

    #[error("SaleExecuted")]
    SaleExecuted {},

    #[error("SaleCancelled")]
    SaleCancelled {},

    #[error("NoTargetADOs")]
    NoTargetADOs {},

    #[error("TokenOwnerCannotBid")]
    TokenOwnerCannotBid {},

    #[error("TokenOwnerCannotBuy")]
    TokenOwnerCannotBuy {},

    #[error("BidSmallerThanHighestBid")]
    BidSmallerThanHighestBid {},

    #[error("Overflow")]
    Overflow {},

    #[error("Underflow")]
    Underflow {},

    #[error("CannotWithdrawHighestBid")]
    CannotWithdrawHighestBid {},

    #[error("WithdrawalIsEmpty")]
    WithdrawalIsEmpty {},

    #[error("AuctionAlreadyStarted")]
    AuctionAlreadyStarted {},

    #[error("StartTimeAfterEndTime")]
    StartTimeAfterEndTime {},

    #[error("Start time in past. Current time: {current_time}. Current block: {current_block}")]
    StartTimeInThePast {
        current_time: u64,
        current_block: u64,
    },

    #[error("OutOfNFTs")]
    OutOfNFTs {},

    #[error("HighestBidderCannotOutBid")]
    HighestBidderCannotOutBid {},

    #[error("InvalidFunds: {msg}")]
    InvalidFunds { msg: String },

    #[error("InvalidPermission: {msg}")]
    InvalidPermission { msg: String },

    #[error("InvalidADOVersion: {msg:?}")]
    InvalidADOVersion { msg: Option<String> },

    #[error("InvalidMinBid: {msg:?}")]
    InvalidMinBid { msg: Option<String> },

    #[error("InvalidCodeID: {msg:?}")]
    InvalidCodeID { msg: Option<String> },

    #[error("InvalidADOType: {msg:?}")]
    InvalidADOType { msg: Option<String> },

    #[error("AuctionRewardAlreadyClaimed")]
    AuctionAlreadyClaimed {},

    #[error("SaleAlreadyConducted")]
    SaleAlreadyConducted {},

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

    #[error("DuplicateDenoms: {denom}")]
    DuplicateDenoms { denom: String },

    #[error("DuplicateThresholds")]
    DuplicateThresholds {},

    #[error("DuplicateRecipient")]
    DuplicateRecipient {},

    #[error("DuplicateContract")]
    DuplicateContract {},

    // BEGIN CW20 ERRORS
    #[error("Cannot set to own account")]
    CannotSetOwnAccount {},

    #[error("Invalid zero amount")]
    InvalidZeroAmount {},

    #[error("Invalid Denom {msg:?}")]
    InvalidDenom { msg: Option<String> },

    #[error("Allowance is expired")]
    Expired {},

    #[error("No allowance for this account")]
    NoAllowance {},

    #[error("Minting cannot exceed the cap")]
    CannotExceedCap {},

    #[error("Logo binary data exceeds 5KB limit")]
    LogoTooBig {},

    #[error("Invalid migration. Unable to migrate from version {prev}")]
    InvalidMigration { prev: String },

    #[error("Invalid xml preamble for SVG")]
    InvalidXmlPreamble {},

    #[error("Invalid png header")]
    InvalidPngHeader {},

    #[error("Instantiate2 Address Mistmatch: expected: {expected}, received: {received}")]
    Instantiate2AddressMismatch { expected: Addr, received: Addr },

    #[error("Duplicate initial balance addresses")]
    DuplicateInitialBalanceAddresses {},

    // END CW20 ERRORS
    #[error("Invalid Module, {msg:?}")]
    InvalidModule { msg: Option<String> },

    #[error("UnsupportedOperation")]
    UnsupportedOperation {},

    #[error("IncompatibleModules: {msg}")]
    IncompatibleModules { msg: String },

    #[error("ModuleDoesNotExist")]
    ModuleDoesNotExist {},

    #[error("token_id already claimed")]
    Claimed {},

    #[error("Approval not found for: {spender}")]
    ApprovalNotFound { spender: String },

    #[error("BidAlreadyPlaced")]
    BidAlreadyPlaced {},

    #[error("BidLowerThanCurrent")]
    BidLowerThanCurrent {},

    #[error("BidNotExpired")]
    BidNotExpired {},

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

    #[error("GeneratorNotSpecified")]
    GeneratorNotSpecified {},

    #[error("TooManyAppComponents")]
    TooManyAppComponents {},

    #[error("InvalidLtvRatio: {msg}")]
    InvalidLtvRatio { msg: String },

    #[error("Name already taken")]
    NameAlreadyTaken {},

    #[error("No Ongoing Sale")]
    NoOngoingSale {},

    #[error("Purchase limit reached")]
    PurchaseLimitReached {},

    #[error("Sale not ended")]
    SaleNotEnded {},

    #[error("Min sales exceeded")]
    MinSalesExceeded {},

    #[error("MinRaiseUnmet")]
    MinRaiseUnmet {},

    #[error("Limit must not be zero")]
    LimitMustNotBeZero {},

    #[error("Sale has already started")]
    SaleStarted {},

    #[error("No purchases")]
    NoPurchases {},

    #[error("Cannot mint after sale conducted")]
    CannotMintAfterSaleConducted {},

    #[error("Not implemented: {msg:?}")]
    NotImplemented { msg: Option<String> },

    #[error("Invalid Strategy: {strategy}")]
    InvalidStrategy { strategy: String },

    #[error("Invalid Query")]
    InvalidQuery {},

    #[error("Unexpected Item Found in: {item}")]
    UnexpectedItem { item: String },

    #[error("Invalid Withdrawal: {msg:?}")]
    InvalidWithdrawal { msg: Option<String> },

    #[error("Airdrop stage {stage} expired at {expiration}")]
    StageExpired { stage: u8, expiration: Milliseconds },

    #[error("Airdrop stage {stage} not expired yet")]
    StageNotExpired { stage: u8, expiration: Milliseconds },

    #[error("Wrong Length")]
    WrongLength {},

    #[error("Verification Failed")]
    VerificationFailed {},

    #[error("Invalid Asset: {asset}")]
    InvalidAsset { asset: String },

    #[error("Asset Error")]
    AssetError {},

    #[error("Invalid cycle duration")]
    InvalidCycleDuration {},

    #[error("Reward increase must be less than 1")]
    InvalidRewardIncrease {},

    #[error("Max of {max} for reward tokens is exceeded")]
    MaxRewardTokensExceeded { max: u32 },

    #[error("Primitive Does Not Exist: {msg}")]
    PrimitiveDoesNotExist { msg: String },

    #[error("Token already being distributed")]
    TokenAlreadyBeingDistributed {},

    #[error("Deposit window closed")]
    DepositWindowClosed {},

    #[error("No saved auction contract")]
    NoSavedBootstrapContract {},

    #[error("Phase ongoing")]
    PhaseOngoing {},

    #[error("Claims already allowed")]
    ClaimsAlreadyAllowed {},

    #[error("ClaimsNotAllowed")]
    ClaimsNotAllowed {},

    #[error("Lockdrop already claimed")]
    LockdropAlreadyClaimed {},

    #[error("No lockup to claim rewards for")]
    NoLockup {},

    #[error("Invalid deposit/withdraw window")]
    InvalidWindow {},

    #[error("Duplicate tokens")]
    DuplicateTokens {},

    #[error("All tokens purchased")]
    AllTokensPurchased {},

    #[error("Token not available")]
    TokenNotAvailable {},

    #[error("Invalid expiration")]
    InvalidExpiration {},

    #[error("Invalid start time")]
    InvalidStartTime {},

    #[error("Too many mint messages, limit is {limit}")]
    TooManyMintMessages { limit: u32 },

    #[error("App contract not specified")]
    AppContractNotSpecified {},

    #[error("VFS contract not specified")]
    VFSContractNotSpecified {},

    #[error("JsonError")]
    JsonError {},

    #[error("Invalid component: {name}")]
    InvalidComponent { name: String },

    #[error("Multi-batch not supported")]
    MultiBatchNotSupported {},

    #[error("Unexpected number of bytes. Expected: {expected}, actual: {actual}")]
    UnexpectedNumberOfBytes { expected: u8, actual: usize },

    #[error("Not an assigned operator, {msg:?}")]
    NotAssignedOperator { msg: Option<String> },

    #[error("Invalid Parameter, {error:?}")]
    InvalidParameter { error: Option<String> },

    #[error("Invalid Pathname, {error:?}")]
    InvalidPathname { error: Option<String> },

    #[error("Invalid Username, {error:?}")]
    InvalidUsername { error: Option<String> },

    #[error("Invalid Packet, {error:?}")]
    InvalidPacket { error: Option<String> },

    #[error("Invalid Denom Trace: {denom}")]
    InvalidDenomTrace { denom: String },

    #[error("Invalid Denom Trace Path: {path}, msg: {msg:?}")]
    InvalidDenomTracePath { path: String, msg: Option<String> },

    #[error("Invalid Expression: {msg}")]
    InvalidExpression { msg: String },

    #[error("Invalid Transfer Port: {port}")]
    InvalidTransferPort { port: String },

    #[error("Invalid Modules: {msg}")]
    InvalidModules { msg: String },

    #[error("Invalid time: {msg}")]
    InvalidTimestamp { msg: String },

    #[error("At least one tier should have no limit")]
    InvalidTiers {},

    #[error("Invalid tier for {operation} operation: {msg} ")]
    InvalidTier { operation: String, msg: String },

    #[error("Environment variable not found: {variable}")]
    EnvironmentVariableNotFound { variable: String },

    #[error("Invalid environment variable length: {msg}")]
    InvalidEnvironmentVariable { msg: String },
}

impl ContractError {
    pub fn new(error: &str) -> Self {
        ContractError::Std(StdError::generic_err(error))
    }
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
            Cw20ContractError::InvalidXmlPreamble {} => ContractError::InvalidXmlPreamble {},
            Cw20ContractError::CannotSetOwnAccount {} => ContractError::CannotSetOwnAccount {},
            Cw20ContractError::DuplicateInitialBalanceAddresses {} => {
                ContractError::DuplicateInitialBalanceAddresses {}
            }
            Cw20ContractError::InvalidExpiration {} => ContractError::InvalidExpiration {},
            _ => panic!("Unsupported cw20 error: {err:?}"),
        }
    }
}

impl From<ParseBoolError> for ContractError {
    fn from(_err: ParseBoolError) -> Self {
        ContractError::ParseBoolError {}
    }
}

impl From<Cw721ContractError> for ContractError {
    fn from(err: Cw721ContractError) -> Self {
        match err {
            Cw721ContractError::Std(std) => ContractError::Std(std),
            Cw721ContractError::Expired {} => ContractError::Expired {},
            Cw721ContractError::Ownership(_) => ContractError::Unauthorized {},
            Cw721ContractError::Claimed {} => ContractError::Claimed {},
            Cw721ContractError::ApprovalNotFound { spender } => {
                ContractError::ApprovalNotFound { spender }
            }
            Cw721ContractError::Version(_) => ContractError::InvalidADOVersion { msg: None },
        }
    }
}

impl From<FromUtf8Error> for ContractError {
    fn from(err: FromUtf8Error) -> Self {
        ContractError::Std(StdError::from(err))
    }
}

impl From<Utf8Error> for ContractError {
    fn from(err: Utf8Error) -> Self {
        ContractError::Std(StdError::from(err))
    }
}

impl From<OverflowError> for ContractError {
    fn from(_err: OverflowError) -> Self {
        ContractError::Overflow {}
    }
}

impl From<AssetError> for ContractError {
    fn from(_err: AssetError) -> Self {
        ContractError::AssetError {}
    }
}

/// Enum that can never be constructed. Used as an error type where we
/// can not error.
#[derive(Error, Debug)]
pub enum Never {}

pub fn from_semver(err: semver::Error) -> StdError {
    StdError::generic_err(format!("Semver: {err}"))
}
