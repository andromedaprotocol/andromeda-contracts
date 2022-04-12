use cosmwasm_std::{
    entry_point, from_binary, Binary, Decimal, Deps, DepsMut, Env, MessageInfo, Response,
    StdResult, Uint128,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use cw_asset::Asset;

use ado_base::ADOContract;
use andromeda_protocol::lockdrop::{
    ConfigResponse, Cw20HookMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, StateResponse,
    UserInfoResponse,
};
use common::{
    ado_base::InstantiateMsg as BaseInstantiateMsg, encode_binary, error::ContractError, require,
};

use crate::state::{Config, State, CONFIG, STATE, USER_INFO};

const UUSD_DENOM: &str = "uusd";

// version info for migration info
const CONTRACT_NAME: &str = "andromeda-lockup";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

//----------------------------------------------------------------------------------------
// Entry Points
//----------------------------------------------------------------------------------------

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // CHECK :: init_timestamp needs to be valid
    require(
        msg.init_timestamp >= env.block.time.seconds(),
        ContractError::StartTimeInThePast {},
    )?;

    // CHECK :: deposit_window,withdrawal_window need to be valid (withdrawal_window < deposit_window)
    require(
        msg.deposit_window > 0
            && msg.withdrawal_window > 0
            && msg.deposit_window < msg.withdrawal_window,
        ContractError::InvalidWindow {},
    )?;

    let config = Config {
        auction_contract_address: msg.auction_contract,
        init_timestamp: msg.init_timestamp,
        deposit_window: msg.deposit_window,
        withdrawal_window: msg.withdrawal_window,
        lockdrop_incentives: Uint128::zero(),
        incentive_token: msg.incentive_token,
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &State::default())?;

    ADOContract::default().instantiate(
        deps.storage,
        deps.api,
        info,
        BaseInstantiateMsg {
            ado_type: "lock_drop".to_string(),
            operators: None,
            modules: None,
            primitive_contract: None,
        },
    )
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AndrReceive(msg) => {
            ADOContract::default().execute(deps, env, info, msg, execute)
        }
        ExecuteMsg::Receive(msg) => receive_cw20(deps, env, info, msg),
        ExecuteMsg::DepositUst {} => try_deposit_ust(deps, env, info),
        ExecuteMsg::WithdrawUst { amount } => try_withdraw_ust(deps, env, info, amount),
        ExecuteMsg::DepositToAuction { amount } => {
            handle_deposit_to_auction(deps, env, info, amount)
        }
        ExecuteMsg::EnableClaims {} => handle_enable_claims(deps, env, info),
        ExecuteMsg::ClaimRewards {} => handle_claim_rewards(deps, env, info),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::default())
}

pub fn receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    cw20_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // CHECK :: Tokens sent > 0
    require(
        !cw20_msg.amount.is_zero(),
        ContractError::InvalidFunds {
            msg: "Number of tokens should be > 0".to_string(),
        },
    )?;

    match from_binary(&cw20_msg.msg)? {
        Cw20HookMsg::IncreaseIncentives {} => {
            handle_increase_incentives(deps, env, info, cw20_msg.amount)
        }
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    match msg {
        QueryMsg::AndrQuery(msg) => ADOContract::default().query(deps, env, msg, query),
        QueryMsg::Config {} => encode_binary(&query_config(deps)?),
        QueryMsg::State {} => encode_binary(&query_state(deps)?),
        QueryMsg::UserInfo { address } => encode_binary(&query_user_info(deps, env, address)?),
        QueryMsg::WithdrawalPercentAllowed { timestamp } => {
            encode_binary(&query_max_withdrawable_percent(deps, env, timestamp)?)
        }
    }
}

//----------------------------------------------------------------------------------------
// Handle Functions
//----------------------------------------------------------------------------------------

/// @dev Facilitates increasing token incentives that are to be distributed as Lockdrop participation reward
/// @params amount : Number of MARS tokens which are to be added to current incentives
pub fn handle_increase_incentives(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    require(
        info.sender == config.incentive_token,
        ContractError::InvalidFunds {
            msg: "Only incentive tokens are valid".to_string(),
        },
    )?;

    let phase_end = config.init_timestamp + config.deposit_window + config.withdrawal_window;

    require(
        env.block.time.seconds() < phase_end,
        ContractError::TokenAlreadyBeingDistributed {},
    )?;

    config.lockdrop_incentives += amount;
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new()
        .add_attribute("action", "incentives_increased")
        .add_attribute("amount", amount))
}

/// @dev Facilitates UST deposits locked for selected number of weeks
/// @param duration : Number of weeks for which UST will be locked
pub fn try_deposit_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let depositor_address = info.sender;

    // CHECK :: Lockdrop deposit window open
    require(
        is_deposit_open(env.block.time.seconds(), &config),
        ContractError::DepositWindowClosed {},
    )?;

    // Check if multiple native coins sent by the user
    require(
        info.funds.len() == 1,
        ContractError::InvalidFunds {
            msg: "Must deposit a single fund".to_string(),
        },
    )?;

    let native_token = info.funds.first().unwrap();
    require(
        native_token.denom == UUSD_DENOM,
        ContractError::InvalidFunds {
            msg: "Only UST accepted".to_string(),
        },
    )?;

    // CHECK ::: Amount needs to be valid
    require(
        !native_token.amount.is_zero(),
        ContractError::InvalidFunds {
            msg: "Amount must be greater than 0".to_string(),
        },
    )?;

    // USER INFO :: RETRIEVE --> UPDATE
    let mut user_info = USER_INFO
        .may_load(deps.storage, &depositor_address)?
        .unwrap_or_default();

    user_info.total_ust_locked += native_token.amount;

    // STATE :: UPDATE --> SAVE
    state.total_ust_locked += native_token.amount;

    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &depositor_address, &user_info)?;

    Ok(Response::new().add_attributes(vec![
        ("action", "lockdrop::ExecuteMsg::lock_ust"),
        ("user", &depositor_address.to_string()),
        ("ust_deposited", native_token.amount.to_string().as_str()),
    ]))
}

/// @dev Facilitates UST withdrawal from an existing Lockup position. Can only be called when deposit / withdrawal window is open
/// @param duration : Duration of the lockup position from which withdrawal is to be made
/// @param withdraw_amount :  UST amount to be withdrawn
pub fn try_withdraw_ust(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    withdraw_amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    let mut user_info = USER_INFO.load(deps.storage, &info.sender)?;

    // USER ADDRESS AND LOCKUP DETAILS
    let withdrawer_address = info.sender;

    // CHECK :: Lockdrop withdrawal window open
    require(
        is_withdraw_open(env.block.time.seconds(), &config),
        ContractError::InvalidWithdrawal {
            msg: Some("Withdrawals not available".to_string()),
        },
    )?;

    // Check :: Amount should be within the allowed withdrawal limit bounds
    let max_withdrawal_percent = allowed_withdrawal_percent(env.block.time.seconds(), &config);
    let max_withdrawal_allowed = user_info.total_ust_locked * max_withdrawal_percent;
    require(
        withdraw_amount <= max_withdrawal_allowed,
        ContractError::InvalidWithdrawal {
            msg: Some(format!(
                "Amount exceeds max allowed withdrawal limit of {}",
                max_withdrawal_allowed
            )),
        },
    )?;

    // Update withdrawal flag after the deposit window
    if env.block.time.seconds() >= config.init_timestamp + config.deposit_window {
        // CHECK :: Max 1 withdrawal allowed
        require(
            !user_info.withdrawal_flag,
            ContractError::InvalidWithdrawal {
                msg: Some("Max 1 withdrawal allowed".to_string()),
            },
        )?;

        user_info.withdrawal_flag = true;
    }

    user_info.total_ust_locked -= withdraw_amount;

    USER_INFO.save(deps.storage, &withdrawer_address, &user_info)?;

    // STATE :: UPDATE --> SAVE
    state.total_ust_locked -= withdraw_amount;
    STATE.save(deps.storage, &state)?;

    // COSMOS_MSG ::TRANSFER WITHDRAWN UST
    let uusd_token = Asset::native(UUSD_DENOM, withdraw_amount);
    let withdraw_msg = uusd_token.transfer_msg(withdrawer_address.clone())?;

    Ok(Response::new()
        .add_messages(vec![withdraw_msg])
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::withdraw_ust"),
            ("user", &withdrawer_address.to_string()),
            ("ust_withdrawn", withdraw_amount.to_string().as_str()),
        ]))
}

/// @dev Function callable only by Auction contract to enable MARS Claims by users. Called along-with Bootstrap Auction contract's LP Pool provide liquidity tx
pub fn handle_enable_claims(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let contract = ADOContract::default();
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // If auction is specified then only it can enable claims.
    if let Some(auction_contract_address) = &config.auction_contract_address {
        let mission_contract = contract.get_mission_contract(deps.storage)?;
        let auction_contract_address =
            auction_contract_address.get_address(deps.api, &deps.querier, mission_contract)?;

        // CHECK :: ONLY AUCTION CONTRACT CAN CALL THIS FUNCTION
        require(
            info.sender == auction_contract_address,
            ContractError::Unauthorized {},
        )?;
    }

    // CHECK :: Claims can only be enabled after the deposit / withdrawal windows are closed
    require(
        !is_withdraw_open(env.block.time.seconds(), &config),
        ContractError::PhaseOngoing {},
    )?;

    // CHECK ::: Claims are only enabled once
    require(
        !state.are_claims_allowed,
        ContractError::ClaimsAlreadyAllowed {},
    )?;
    state.are_claims_allowed = true;

    STATE.save(deps.storage, &state)?;
    Ok(Response::new().add_attribute("action", "Lockdrop::ExecuteMsg::EnableClaims"))
}

/// @dev Function to delegate part of the MARS rewards to be used for LP Bootstrapping via auction
/// @param amount : Number of MARS to delegate
pub fn handle_deposit_to_auction(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;
    let user_address = info.sender;

    let phase_end = config.init_timestamp + config.deposit_window + config.withdrawal_window;
    // CHECK :: Have the deposit / withdraw windows concluded
    require(
        env.block.time.seconds() >= phase_end,
        ContractError::PhaseOngoing {},
    )?;

    // CHECK :: Can users withdraw their MARS tokens ? -> if so, then delegation is no longer allowed
    require(
        !state.are_claims_allowed,
        ContractError::ClaimsAlreadyAllowed {},
    )?;

    // CHECK :: Auction contract address should be set
    require(
        config.auction_contract_address.is_some(),
        ContractError::NoSavedAuctionContract {},
    )?;

    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    let total_incentives = config
        .lockdrop_incentives
        .multiply_ratio(user_info.total_ust_locked, state.total_ust_locked);

    // CHECK :: MARS to delegate cannot exceed user's unclaimed MARS balance
    let available_amount = total_incentives - user_info.delegated_mars_incentives;
    require(
        amount <= available_amount,
        ContractError::InvalidFunds {
            msg: format!(
                "Amount cannot exceed user's unclaimed MARS balance. MARS to delegate = {}, Max delegatable MARS = {} ",
                amount,
                available_amount
            ),
        },
    )?;

    // UPDATE STATE
    user_info.delegated_mars_incentives += amount;
    state.total_mars_delegated += amount;

    // SAVE UPDATED STATE
    STATE.save(deps.storage, &state)?;
    USER_INFO.save(deps.storage, &user_address, &user_info)?;

    // COSMOS_MSG ::Delegate MARS to the LP Bootstrapping via Auction contract
    // TODO: When Boostrapping contract is created add this message.

    Ok(Response::new()
        .add_attribute("action", "Auction::ExecuteMsg::DelegateMarsToAuction")
        .add_attribute("user_address", &user_address.to_string())
        .add_attribute("delegated_mars", amount.to_string()))
}

/// @dev Function to claim Rewards and optionally unlock a lockup position (either naturally or forcefully). Claims pending incentives (xMARS) internally and accounts for them via the index updates
/// @params lockup_to_unlock_duration : Duration of the lockup to be unlocked. If 0 then no lockup is to be unlocked
pub fn handle_claim_rewards(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    let user_address = info.sender;
    let mut user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    require(
        !user_info.lockdrop_claimed,
        ContractError::LockdropAlreadyClaimed {},
    )?;
    require(
        !user_info.total_ust_locked.is_zero(),
        ContractError::NoLockup {},
    )?;
    require(state.are_claims_allowed, ContractError::ClaimsNotAllowed {})?;

    let total_incentives = config
        .lockdrop_incentives
        .multiply_ratio(user_info.total_ust_locked, state.total_ust_locked);

    let amount_to_transfer = total_incentives - user_info.delegated_mars_incentives;
    let token = Asset::cw20(
        deps.api.addr_validate(&config.incentive_token)?,
        amount_to_transfer,
    );
    let transfer_msg = token.transfer_msg(user_address.clone())?;
    user_info.lockdrop_claimed = true;

    USER_INFO.save(deps.storage, &user_address, &user_info)?;

    Ok(Response::new()
        .add_attribute(
            "action",
            "Auction::ExecuteMsg::ClaimRewardsAndUnlockPosition",
        )
        .add_message(transfer_msg))
}

pub fn try_withdraw_proceeds(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    // CHECK :: Only Owner can call this function
    require(
        ADOContract::default().is_contract_owner(deps.storage, info.sender.as_str())?,
        ContractError::Unauthorized {},
    )?;

    // CHECK :: Lockdrop withdrawal window should be closed
    let current_timestamp = env.block.time.seconds();
    require(
        current_timestamp >= config.init_timestamp && !is_withdraw_open(current_timestamp, &config),
        ContractError::InvalidWithdrawal {
            msg: Some("Lockdrop withdrawals haven't concluded yet".to_string()),
        },
    )?;

    let uusd_token = Asset::native(UUSD_DENOM, state.total_ust_locked);

    let balance = uusd_token
        .info
        .query_balance(&deps.querier, env.contract.address)?;
    require(
        balance >= state.total_ust_locked,
        ContractError::InvalidWithdrawal {
            msg: Some("Already withdrew funds".to_string()),
        },
    )?;

    let transfer_msg = uusd_token.transfer_msg(recipient)?;

    Ok(Response::new()
        .add_message(transfer_msg)
        .add_attributes(vec![
            ("action", "lockdrop::ExecuteMsg::DepositInRedBank"),
            (
                "ust_deposited_in_red_bank",
                state.total_ust_locked.to_string().as_str(),
            ),
            ("timestamp", env.block.time.seconds().to_string().as_str()),
        ]))
}

//----------------------------------------------------------------------------------------
// Query Functions
//----------------------------------------------------------------------------------------

/// @dev Returns the contract's configuration
pub fn query_config(deps: Deps) -> Result<ConfigResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let contract = ADOContract::default();
    let mission_contract = contract.get_mission_contract(deps.storage)?;
    let auction_contract_address = config
        .auction_contract_address
        .map(|a| a.get_address(deps.api, &deps.querier, mission_contract))
        // Flip Option<Result> to Result<Option>
        .map_or(Ok(None), |v| v.map(Some));

    Ok(ConfigResponse {
        auction_contract_address: auction_contract_address?,
        init_timestamp: config.init_timestamp,
        deposit_window: config.deposit_window,
        withdrawal_window: config.withdrawal_window,
        lockdrop_incentives: config.lockdrop_incentives,
    })
}

/// @dev Returns the contract's Global State
pub fn query_state(deps: Deps) -> Result<StateResponse, ContractError> {
    let state: State = STATE.load(deps.storage)?;
    Ok(StateResponse {
        total_ust_locked: state.total_ust_locked,
        total_mars_delegated: state.total_mars_delegated,
        are_claims_allowed: state.are_claims_allowed,
    })
}

/// @dev Returns summarized details regarding the user
/// @params user_address : User address whose state is being queries
pub fn query_user_info(
    deps: Deps,
    _env: Env,
    user_address_: String,
) -> Result<UserInfoResponse, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let user_address = deps.api.addr_validate(&user_address_)?;
    let state: State = STATE.load(deps.storage)?;
    let user_info = USER_INFO
        .may_load(deps.storage, &user_address)?
        .unwrap_or_default();

    let total_incentives = config
        .lockdrop_incentives
        .multiply_ratio(user_info.total_ust_locked, state.total_ust_locked);

    Ok(UserInfoResponse {
        total_ust_locked: user_info.total_ust_locked,
        total_mars_incentives: total_incentives,
        delegated_mars_incentives: user_info.delegated_mars_incentives,
        is_lockdrop_claimed: user_info.lockdrop_claimed,
    })
}

/// @dev Returns max withdrawable % for a position
pub fn query_max_withdrawable_percent(
    deps: Deps,
    env: Env,
    timestamp: Option<u64>,
) -> Result<Decimal, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    Ok(match timestamp {
        Some(timestamp) => allowed_withdrawal_percent(timestamp, &config),
        None => allowed_withdrawal_percent(env.block.time.seconds(), &config),
    })
}

//----------------------------------------------------------------------------------------
// HELPERS
//----------------------------------------------------------------------------------------

/// @dev Returns true if deposits are allowed
fn is_deposit_open(current_timestamp: u64, config: &Config) -> bool {
    let deposits_opened_till = config.init_timestamp + config.deposit_window;
    (current_timestamp >= config.init_timestamp) && (deposits_opened_till >= current_timestamp)
}

/// @dev Returns true if withdrawals are allowed
fn is_withdraw_open(current_timestamp: u64, config: &Config) -> bool {
    let withdrawals_opened_till =
        config.init_timestamp + config.deposit_window + config.withdrawal_window;
    (current_timestamp >= config.init_timestamp) && (withdrawals_opened_till >= current_timestamp)
}

/// @dev Helper function to calculate maximum % of UST deposited that can be withdrawn
/// @params current_timestamp : Current block timestamp
/// @params config : Contract configuration
fn allowed_withdrawal_percent(current_timestamp: u64, config: &Config) -> Decimal {
    let withdrawal_cutoff_init_point = config.init_timestamp + config.deposit_window;

    // Deposit window :: 100% withdrawals allowed
    if current_timestamp < withdrawal_cutoff_init_point {
        return Decimal::from_ratio(100u32, 100u32);
    }

    let withdrawal_cutoff_second_point =
        withdrawal_cutoff_init_point + (config.withdrawal_window / 2u64);
    // Deposit window closed, 1st half of withdrawal window :: 50% withdrawals allowed
    if current_timestamp <= withdrawal_cutoff_second_point {
        return Decimal::from_ratio(50u32, 100u32);
    }

    // max withdrawal allowed decreasing linearly from 50% to 0% vs time elapsed
    let withdrawal_cutoff_final = withdrawal_cutoff_init_point + config.withdrawal_window;
    //  Deposit window closed, 2nd half of withdrawal window :: max withdrawal allowed decreases linearly from 50% to 0% vs time elapsed
    if current_timestamp < withdrawal_cutoff_final {
        let time_left = withdrawal_cutoff_final - current_timestamp;
        Decimal::from_ratio(
            50u64 * time_left,
            100u64 * (withdrawal_cutoff_final - withdrawal_cutoff_second_point),
        )
    }
    // Withdrawals not allowed
    else {
        Decimal::from_ratio(0u32, 100u32)
    }
}
