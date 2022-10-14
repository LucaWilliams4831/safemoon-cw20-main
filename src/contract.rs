#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env,
    MessageInfo, QuerierWrapper, Response, StdError, StdResult, Uint128, WasmMsg, Storage, Order, Api, attr, QueryRequest, BankQuery, CosmosMsg, WasmQuery, Coin,
    BalanceResponse, SubMsg
};

use wasmswap::msg::{InfoResponse, ExecuteMsg as WasmswapExecuteMsg, QueryMsg as WasmswapQueryMsg, TokenSelect};

use cw2::set_contract_version;
use cw20_base::allowances::{
    execute_burn_from, execute_decrease_allowance, execute_increase_allowance, execute_send_from,
    execute_transfer_from, query_allowance, deduct_allowance
};

use cw20_base::state::{MinterData, TokenInfo, TOKEN_INFO};
use cw20::{TokenInfoResponse, BalanceResponse as CW20BalanceResponse, Cw20ReceiveMsg, Cw20QueryMsg, Cw20ExecuteMsg};
use crate::error::ContractError;
use crate::msg::{ConfigResponse, ExecuteMsg, InstantiateMsg, QueryMsg, ExcludedInfo, BoolResponse, Uint128Response};
use crate::state::{Config, CONFIG, ROWNED, TOWNED, ISEXCLUDEDFROMFEE, ISEXCLUDED};


// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw20-safemoon";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    
    // multiple values for every size value
    let multiply = Uint128::from(1u128);
    // Set the token supply to 1000M = 1G
    // i.e. 100 * 1000_000 * 10^decimals
    
    let t_total = multiply * Uint128::from(1000u128 * 1000000u128 * 10u128.pow(msg.decimals as u32));
    let r_total = Uint128::MAX - (Uint128::MAX % t_total);
    ROWNED.save(deps.storage, info.clone().sender.clone(), &r_total)?;
    ISEXCLUDEDFROMFEE.save(deps.storage, info.clone().sender.clone(), &true)?;
    ISEXCLUDEDFROMFEE.save(deps.storage, env.contract.address.clone(), &true)?;

    let config = Config {
        owner: info.clone().sender.clone(),
        t_total,
        r_total,
        t_fee_total: Uint128::zero(),
        tax_fee: msg.tax_fee,
        previous_tax_fee: msg.tax_fee,
        liquidity_fee: msg.liquidity_fee,
        previous_liquidity_fee: msg.liquidity_fee,
        pool_address: msg.pool_address,
        swap_and_liquidity_enabled: true,
        // set max_tx_amount to 100k
        max_tx_amount: multiply * Uint128::from(100u128 * 1000u128 * 10u128.pow(msg.decimals as u32)),
        // set num_tokens_sell_to_add_to_liquidity to 5K
        num_tokens_sell_to_add_to_liquidity: multiply * Uint128::from(5u128 * 1000u128 * 10u128.pow(msg.decimals as u32)),
        denom: msg.denom
    };
    
    CONFIG.save(deps.storage, &config)?;

    // store token info using cw20-base format
    let data = TokenInfo {
        name: msg.name,
        symbol: msg.symbol,
        decimals: msg.decimals,
        total_supply: t_total,
        // set self as minter, so we can properly execute mint and burn
        mint: Some(MinterData {
            minter: env.contract.address,
            cap: None,
        }),
    };
    TOKEN_INFO.save(deps.storage, &data)?;

    // Ok(Response::default())
    Ok(Response::new().add_attribute("action", "instantiate")
    .add_attribute("recipient", info.sender.clone())
    .add_attribute("amount", t_total))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig { new_owner } => update_config(deps, info, new_owner),
        ExecuteMsg::UpdatePool { address } => update_pool(deps, info, address),
        // these all come from cw20-base to implement the cw20 standard
        ExecuteMsg::Transfer { recipient, amount } => {
            execute_custom_transfer(deps, env, info, recipient, amount)
        },
        ExecuteMsg::Send {
            contract,
            amount,
            msg,
        } => Ok(execute_custom_send(deps, env, info, contract, amount, msg)?),
        ExecuteMsg::IncreaseAllowance {
            spender,
            amount,
            expires,
        } => Ok(execute_increase_allowance(
            deps, env, info, spender, amount, expires,
        )?),
        ExecuteMsg::DecreaseAllowance {
            spender,
            amount,
            expires,
        } => Ok(execute_decrease_allowance(
            deps, env, info, spender, amount, expires,
        )?),
        ExecuteMsg::TransferFrom {
            owner,
            recipient,
            amount,
        } => Ok(execute_custom_transfer_from(
            deps, env, info, owner, recipient, amount,
        )?),
        ExecuteMsg::SendFrom {
            owner,
            contract,
            amount,
            msg,
        } => Ok(execute_custom_send_from(
            deps, env, info, owner, contract, amount, msg,
        )?),
        ExecuteMsg::Deliver {
            amount
        } => execute_deliver(deps, env, info, amount),

        ExecuteMsg::ExcludeFromReward {
            address
        } => execute_exclude_from_reward(deps, env, info, address),
        ExecuteMsg::IncludeInReward {
            address
        } => execute_include_in_reward(deps, env, info, address),
        ExecuteMsg::ExcludeFromFee {
            address
        } => execute_exclude_from_fee(deps, env, info, address),
        ExecuteMsg::IncludeInFee {
            address
        } => execute_include_in_fee(deps, env, info, address),
        ExecuteMsg::SetTaxFeePercent {
            percent
        } => execute_set_tax_fee_percent(deps, env, info, percent),
        ExecuteMsg::SetLiquidityFeePercent {
            percent
        } => execute_set_liquidity_fee_percent(deps, env, info, percent),
        ExecuteMsg::SetMaxTxPercent {
            percent
        } => execute_set_max_tx_percent(deps, env, info, percent),
        ExecuteMsg::SetSwapAndLiquifyEnabled {
            enabled
        } => execute_set_swap_and_liquify_enabled(deps, env, info, enabled),
        ExecuteMsg::FetchAdmin {} => execute_fetch_admin(deps, env, info)
        
    }
}


pub fn execute_fetch_admin(
    deps: DepsMut,
    env: Env,
    info: MessageInfo
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;

    let cfg = CONFIG.load(deps.storage)?;

    let contract_address = env.clone().contract.address.clone();
    // Get LP token address
    let pool_response: InfoResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: cfg.pool_address.clone().into(),
        msg: to_binary(&WasmswapQueryMsg::Info {})?,
    }))?;
    let lp_address = pool_response.lp_token_address;

    //Get LP balance
    let balance_response: CW20BalanceResponse = deps.querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: lp_address.clone().into(),
        msg: to_binary(&Cw20QueryMsg::Balance {address: contract_address.clone().into()})?,
    }))?;
    let lp_balance = balance_response.balance;
    
    //Get Juno Balance
    let juno_response: BalanceResponse = deps.querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: contract_address.clone().into(),
        denom: cfg.denom.clone()
    }))?;
    let juno_balance = juno_response.amount.amount;

    // create transfer cw20 msg
    let transfer_msg = Cw20ExecuteMsg::Transfer {
        recipient: info.sender.clone().into(),
        amount: lp_balance,
    };
    let exec_transfer = WasmMsg::Execute {
        contract_addr: lp_address.clone().into(),
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    };
    
    let cw20_send_cosmos_msg: CosmosMsg = exec_transfer.into();


    let transfer_bank_msg = cosmwasm_std::BankMsg::Send {
        to_address: info.sender.clone().into(),
        amount: vec![Coin{amount: juno_balance, denom: cfg.denom.clone()}],
    };
    let transfer_bank_cosmos_msg: CosmosMsg = transfer_bank_msg.into();

    
    

    Ok(Response::new().add_attribute("action", "fetch_admin")
    .add_submessages(vec![SubMsg::new(cw20_send_cosmos_msg), SubMsg::new(transfer_bank_cosmos_msg)]))

}
pub fn execute_exclude_from_reward(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: Addr
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;
    
    if ISEXCLUDED.load(deps.storage, address.clone()).unwrap_or(false) {
        return Err(ContractError::AccountAlreadyExcluded {})
    }

    let r_val = ROWNED.load(deps.storage, address.clone()).unwrap_or(Uint128::zero());
    if  r_val > Uint128::zero() {
        let t_val = _token_from_reflection(deps.storage, r_val);
        TOWNED.save(deps.storage, address.clone(), &t_val)?;
    }

    ISEXCLUDED.save(deps.storage, address.clone(), &true)?;

    Ok(Response::new()
        .add_attribute("action", "exclude_from_reward")
        .add_attribute("address", address.clone())
    )
}

pub fn execute_include_in_reward(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: Addr
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;
    
    if !ISEXCLUDED.load(deps.storage, address.clone()).unwrap_or(false) {
        return Err(ContractError::AccountAlreadyExcluded {})
    }

    let r_val = ROWNED.load(deps.storage, address.clone()).unwrap_or(Uint128::zero());
    if  r_val > Uint128::zero() {
        let t_val = _token_from_reflection(deps.storage, r_val);
        TOWNED.save(deps.storage, address.clone(), &t_val)?;
    }

    ISEXCLUDED.save(deps.storage, address.clone(), &true)?;

    Ok(Response::new()
        .add_attribute("action", "include_in_reward")
        .add_attribute("address", address.clone())
    )
}

pub fn execute_exclude_from_fee(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: Addr
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;
    
    ISEXCLUDEDFROMFEE.save(deps.storage, address.clone(), &true)?;
    
    Ok(Response::new()
        .add_attribute("action", "exclude_from_fee")
        .add_attribute("address", address.clone())
    )
}

pub fn execute_include_in_fee(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    address: Addr
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;
    
    ISEXCLUDEDFROMFEE.save(deps.storage, address.clone(), &false)?;
    
    Ok(Response::new()
        .add_attribute("action", "include_in_fee")
        .add_attribute("address", address.clone())
    )
}

pub fn execute_set_tax_fee_percent(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    percent: u8
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;
    
    let mut cfg = CONFIG.load(deps.storage)?;
    cfg.tax_fee = percent;
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "set_tax_fee_percent")
        .add_attribute("precent", Uint128::from(percent))
    )
}

pub fn execute_set_liquidity_fee_percent(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    percent: u8
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;
    
    let mut cfg = CONFIG.load(deps.storage)?;
    cfg.liquidity_fee = percent;
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "set_liquidity_fee_percent")
        .add_attribute("percent", Uint128::from(percent))
    )
}

pub fn execute_set_max_tx_percent(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    percent: u8
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;
    
    let mut cfg = CONFIG.load(deps.storage)?;
    cfg.max_tx_amount = cfg.t_total * Uint128::from(percent as u32) / Uint128::from(100u128);
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "set_max_tx_percent")
        .add_attribute("percent", Uint128::from(percent))
    )
}

pub fn execute_set_swap_and_liquify_enabled(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    enabled: bool
) -> Result<Response, ContractError> {

    _check_owner(&deps, &info)?;
    
    let mut cfg = CONFIG.load(deps.storage)?;
    cfg.swap_and_liquidity_enabled = enabled;
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "set_swap_and_liquify_enabled")
        
    )
}

pub fn execute_deliver(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    t_amount: Uint128
) -> Result<Response, ContractError> {
    
    let mut cfg = CONFIG.load(deps.storage)?;
    if !ISEXCLUDED.load(deps.storage, info.sender.clone()).unwrap_or(false) {
        return Err(ContractError::ExcludedDisableDeliver {})
    }

    let (r_amount, _r_transfer_amount, _r_fee, _t_transfer_amount, _t_fee, _t_liquidity) = _get_values(deps.storage, t_amount);
    let r_val = ROWNED.load(deps.storage, info.sender.clone()).unwrap_or(Uint128::zero());
    ROWNED.save(deps.storage, info.sender.clone(), &(r_val - r_amount))?;

    cfg.r_total -= r_amount;
    cfg.t_fee_total += t_amount;
    CONFIG.save(deps.storage, &cfg)?;

    Ok(Response::new()
        .add_attribute("action", "deliver")
        .add_attribute("t_amount", t_amount)
    )
}

pub fn _check_owner(
    deps: &DepsMut,
    info: &MessageInfo
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    if info.sender != cfg.owner {
        return Err(ContractError::Unauthorized {})
    }
    Ok(Response::new().add_attribute("action", "check_owner"))
}


pub fn update_config(
    deps: DepsMut,
    info: MessageInfo,
    new_owner: String,
) -> Result<Response, ContractError> {
    // authorize owner
    _check_owner(&deps, &info)?;

    let new_addr = deps.api.addr_validate(&new_owner)?;

    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.owner = new_addr.clone();
        Ok(exists)
    })?;

    Ok(Response::new().add_attribute("action", "update_config").add_attribute("owner", new_owner))
}

pub fn update_pool(
    deps: DepsMut,
    info: MessageInfo,
    address: Addr,
) -> Result<Response, ContractError> {
    // authorize owner
    _check_owner(&deps, &info)?;

    CONFIG.update(deps.storage, |mut exists| -> StdResult<_> {
        exists.pool_address = address.clone();
        Ok(exists)
    })?;

    Ok(Response::new().add_attribute("action", "update_pool").add_attribute("address", address.clone()))
}

pub fn _calculate_tax_fee(storage: &mut dyn Storage, amount: Uint128) -> Uint128 {
    let cfg = CONFIG.load(storage).unwrap();
    return amount * Uint128::from(cfg.tax_fee as u128) / Uint128::from(100u128);
}

pub fn _calculate_liquidity_fee(storage: &mut dyn Storage, amount: Uint128) -> Uint128 {
    let cfg = CONFIG.load(storage).unwrap();
    return amount * Uint128::from(cfg.liquidity_fee as u128) / Uint128::from(100u128);
}

pub fn _get_t_values(storage: &mut dyn Storage, t_amount: Uint128) -> (Uint128, Uint128, Uint128) {
    let t_fee = _calculate_tax_fee(storage, t_amount);
    let t_liquidity = _calculate_liquidity_fee(storage, t_amount);
    let t_transfer_amount = t_amount - t_fee - t_liquidity;
    return (t_transfer_amount, t_fee, t_liquidity);
}

pub fn _get_r_values(t_amount: Uint128, t_fee: Uint128, t_liquidity: Uint128, current_rate: Uint128) -> (Uint128, Uint128, Uint128) {
    let r_amount = t_amount * current_rate;
    let r_fee = t_fee * current_rate;
    let r_liquidity = t_liquidity * current_rate;
    let r_transfer_amount = r_amount - r_fee - r_liquidity;
    return (r_amount, r_transfer_amount, r_fee);
}

pub fn _get_values(storage: &mut dyn Storage, t_amount: Uint128) -> (Uint128, Uint128, Uint128, Uint128, Uint128, Uint128) {
    let (t_transfer_amount, t_fee, t_liquidity) = _get_t_values(storage, t_amount);
    let current_rate = _get_rate(storage);
    let (r_amount, r_transfer_amount, r_fee) = _get_r_values( t_amount, t_fee, t_liquidity, current_rate);
    return (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity);
}

pub fn _take_liquidity(storage: &mut dyn Storage, env: Env, t_liquidity: Uint128) -> StdResult<Response> {
    let current_rate =  _get_rate(storage);
    let r_liquidity = t_liquidity * current_rate;
    let contract_addr = env.contract.address.clone();
    let r_val = ROWNED.load(storage, contract_addr.clone()).unwrap_or(Uint128::zero()) + r_liquidity;
    ROWNED.save(storage, contract_addr.clone(), &r_val)?;

    if  ISEXCLUDED.load(storage, contract_addr.clone()).unwrap_or(false) {
        let t_val = TOWNED.load(storage, contract_addr.clone()).unwrap_or(Uint128::zero()) + t_liquidity;
        TOWNED.save(storage, contract_addr.clone(), &t_val)?;
    }
    Ok(Response::default())
    
}

pub fn _reflect_fee(storage: &mut dyn Storage, r_fee: Uint128, t_fee: Uint128) -> StdResult<Response> {
    let mut cfg = CONFIG.load(storage)?;
    cfg.r_total -= r_fee;
    cfg.t_fee_total += t_fee;
    CONFIG.save(storage, &cfg)?;
    Ok(Response::default())
}

//swap and liquify

pub fn swap_and_liquify(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: Env,
    contract_token_balance: Uint128
) -> Result<Vec<CosmosMsg>, ContractError> {
    // split the contract balance into halves
    let half = contract_token_balance / Uint128::from(2u128);
    let other_half = contract_token_balance - half;

    let mut messages: Vec<CosmosMsg> = vec![];
    // capture the contract's current Juno balance.
    // this is so that we can capture exactly the amount of Juno that the
    // swap creates, and not make the liquidity event include any Juno that
    // has been manually sent to the contract
    let cfg = CONFIG.load(storage)?;
    let contract_addr = env.contract.address.clone();
    let native_balance: BalanceResponse = querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: contract_addr.clone().into(),
        denom: cfg.denom.clone()
    }))?;
    let initial_balance = native_balance.amount.amount;

    // swap tokens for Juno
    // swap_tokens_for_juno(storage, querier, env.clone(), half)?;
    // {
        let cfg = CONFIG.load(storage)?;
        // generate the uniswap pair path of token -> juno
        //query token1.reserve and token2.reserve
        let info_response: InfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
            contract_addr: cfg.pool_address.clone().into(),
            msg: to_binary(&WasmswapQueryMsg::Info {})?,
        }))?;

        //Increase allowance
        let increase_allowance_swap_msg = ExecuteMsg::IncreaseAllowance {
            spender: cfg.pool_address.clone().into(),
            amount: half,
            expires: None 
        };
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.clone().into(),
            msg: to_binary(&increase_allowance_swap_msg)?,
            funds: vec![],
        }));

        //input token is token2
        let juno_bought = _get_input_price(half, info_response.token2_reserve, info_response.token1_reserve)?;
        // Juno is Token1, Safemoon is Token2
        let swap_msg = WasmswapExecuteMsg::Swap {
            input_token: TokenSelect::Token2,
            input_amount: half,
            min_output: juno_bought,
            expiration: None
        };
        let callback = CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.pool_address.clone().into(),
            msg: to_binary(&swap_msg)?,
            funds: vec![],
        });
        messages.push(callback);
    // }

    // how much juno did we just swap into?
    // let native_balance_new: BalanceResponse = querier.query(&QueryRequest::Bank(BankQuery::Balance {
    //     address: contract_addr.clone().into(),
    //     denom: cfg.denom.clone()
    // }))?;
    // let new_balance = native_balance_new.amount.amount - initial_balance;

    // add liquidity to uniswap
    // add_liquidity(storage, querier, env.clone(), other_half, new_balance)?;
    // {
        let cfg = CONFIG.load(storage)?;

        

        let token2_amount = _get_token2_amount_required(
            juno_bought,
            info_response.token2_reserve + half,
            info_response.token1_reserve - juno_bought
        )?;

        let increase_allowance_msg = ExecuteMsg::IncreaseAllowance {
            spender: cfg.pool_address.clone().into(),
            amount: token2_amount,
            expires: None 
        };
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.clone().into(),
            msg: to_binary(&increase_allowance_msg)?,
            funds: vec![],
        }));
    
        // let max_token2 = _get_input_price(juno_bought, info_response.token1_reserve, info_response.token2_reserve)?;
        let add_liquidity_msg = WasmswapExecuteMsg::AddLiquidity {
            token1_amount: juno_bought,
            max_token2: token2_amount,
            min_liquidity: Uint128::from(1u128),
            expiration: None
        };
        let funds = Coin {
            amount: juno_bought,
            denom: cfg.denom,
        };
        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: cfg.pool_address.clone().into(),
            msg: to_binary(&add_liquidity_msg)?,
            funds: vec![funds],
        }));

    // }
    Ok(messages)
    
    // Ok(Response::new()
    //     .add_messages(messages)
    //     .add_attribute("action", "swap_and_liquify")
    //     .add_attribute("contract_token_balance", contract_token_balance)
    // )
}


fn _get_input_price(
    input_amount: Uint128,
    input_reserve: Uint128,
    output_reserve: Uint128,
) -> StdResult<Uint128> {
    if input_reserve == Uint128::zero() || output_reserve == Uint128::zero() {
        return Err(StdError::generic_err("No liquidity"));
    };

    let input_amount_with_fee = input_amount
        .checked_mul(Uint128::new(997))
        .map_err(StdError::overflow)?;
    let numerator = input_amount_with_fee
        .checked_mul(output_reserve)
        .map_err(StdError::overflow)?;
    let denominator = input_reserve
        .checked_mul(Uint128::new(1000))
        .map_err(StdError::overflow)?
        .checked_add(input_amount_with_fee)
        .map_err(StdError::overflow)?;

    numerator
        .checked_div(denominator)
        .map_err(StdError::divide_by_zero)
}

fn _get_token2_amount_required(
    token1_amount: Uint128,
    token2_reserve: Uint128,
    token1_reserve: Uint128,
) -> Result<Uint128, StdError> {
    Ok(token1_amount
        .checked_mul(token2_reserve)
        .map_err(StdError::overflow)?
        .checked_div(token1_reserve)
        .map_err(StdError::divide_by_zero)?
        .checked_add(Uint128::new(1))
        .map_err(StdError::overflow)?)
}
pub fn swap_tokens_for_juno(
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    _env: Env,
    token_amount: Uint128
) -> StdResult<Response> {
    let cfg = CONFIG.load(storage)?;
    // generate the uniswap pair path of token -> juno
    //query token1.reserve and token2.reserve
    let info_response: InfoResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: cfg.pool_address.clone().into(),
        msg: to_binary(&WasmswapQueryMsg::Info {})?,
    }))?;
    //input token is token2
    let token_bought = _get_input_price(token_amount, info_response.token2_reserve, info_response.token1_reserve)?;
    // Juno is Token1, Safemoon is Token2
    let swap_msg = WasmswapExecuteMsg::Swap {
        input_token: TokenSelect::Token2,
        input_amount: token_amount,
        min_output: token_bought,
        expiration: None
    };
    let callback = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.pool_address.clone().into(),
        msg: to_binary(&swap_msg)?,
        funds: vec![],
    });

    Ok(Response::new().add_attribute("action", "swap_tokens_for_juno").add_message(callback))
}

pub fn add_liquidity(
    storage: &mut dyn Storage,
    _querier: QuerierWrapper,
    env: Env,
    token_amount: Uint128,
    juno_amount: Uint128
) -> StdResult<Response> {
    let cfg = CONFIG.load(storage)?;

    let mut messages:Vec<CosmosMsg> = vec![];
    let increase_allowance_msg = ExecuteMsg::IncreaseAllowance {
        spender: cfg.pool_address.clone().into(),
        amount: token_amount,
        expires: None 
    };
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: env.contract.address.clone().into(),
        msg: to_binary(&increase_allowance_msg)?,
        funds: vec![],
    }));

    let add_liquidity_msg = WasmswapExecuteMsg::AddLiquidity {
        token1_amount: juno_amount,
        max_token2: token_amount,
        min_liquidity: Uint128::from(1u128),
        expiration: None
    };
    let funds = Coin {
        amount: juno_amount,
        denom: cfg.denom,
    };
    messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: cfg.pool_address.clone().into(),
        msg: to_binary(&add_liquidity_msg)?,
        funds: vec![funds],
    }));

    
    Ok(Response::new().add_attribute("action", "add_liquidity").add_messages(messages))
}

pub fn _transfer(
    _api: & dyn Api,
    storage: &mut dyn Storage,
    querier: QuerierWrapper,
    env: Env,
    sender: Addr,
    recipient: Addr,
    amount: Uint128,
    in_swap_and_liquify: bool
) -> Result<Vec<CosmosMsg>, ContractError> {
    let mut cfg = CONFIG.load(storage)?;
    if cfg.owner != sender && cfg.owner != recipient && amount > cfg.max_tx_amount {
        return Err(ContractError::MaxTxAmountExceed {});
    }

    let mut msgs: Vec<CosmosMsg> = vec![];

    // balance of this contract
    let mut contract_token_balance;
    let contract_addr = env.contract.address.clone();
    if ISEXCLUDED.load(storage, contract_addr.clone()).unwrap_or(false) {
        contract_token_balance = TOWNED.load(storage, contract_addr.clone()).unwrap_or(Uint128::zero());
    } else {
        let r_amount = ROWNED.load(storage, contract_addr.clone()).unwrap_or(Uint128::zero());
        contract_token_balance = _token_from_reflection(storage, r_amount);
    }
    // balance end
    if contract_token_balance >= cfg.max_tx_amount {
        contract_token_balance = cfg.max_tx_amount;
    }

    let over_min_token_balance = contract_token_balance >= cfg.num_tokens_sell_to_add_to_liquidity;
    let mut messages: Vec<CosmosMsg> = vec![];
    if over_min_token_balance && !in_swap_and_liquify && sender != cfg.pool_address && cfg.swap_and_liquidity_enabled {
        contract_token_balance = cfg.num_tokens_sell_to_add_to_liquidity;
        messages = swap_and_liquify(storage, querier, env.clone(), contract_token_balance)?;
    }
    let mut take_fee = true;
    if ISEXCLUDEDFROMFEE.load(storage, recipient.clone()).unwrap_or(false) || ISEXCLUDEDFROMFEE.load(storage, sender.clone()).unwrap_or(false) {
        take_fee = false;
    }

    if !take_fee {
        if !(cfg.tax_fee == 0u8 && cfg.liquidity_fee == 0u8) {
            cfg.previous_tax_fee = cfg.tax_fee;
            cfg.previous_liquidity_fee = cfg.liquidity_fee;
            
            cfg.tax_fee = 0u8;
            cfg.liquidity_fee = 0u8;
            CONFIG.save(storage, &cfg)?;
        }
    }
    
    let sender_excluded = ISEXCLUDED.load(storage, sender.clone()).unwrap_or(false);
    let recipient_excluded = ISEXCLUDED.load(storage, recipient.clone()).unwrap_or(false);
    
    if sender_excluded && !recipient_excluded {
        _transfer_from_excluded(storage, env, sender, recipient, amount)?;
    } else if !sender_excluded && recipient_excluded {
        _transfer_to_excluded(storage, env, sender, recipient, amount)?;
    } else if !sender_excluded && !recipient_excluded {
        _transfer_standard(storage, env, sender, recipient, amount)?;
    } else if sender_excluded && recipient_excluded {
        _transfer_both_excluded(storage, env, sender, recipient, amount)?;
    } else {
        _transfer_standard(storage, env, sender, recipient, amount)?;
    }

    if !take_fee {
        cfg.tax_fee = cfg.previous_tax_fee;
        cfg.liquidity_fee = cfg.previous_liquidity_fee;
        CONFIG.save(storage, &cfg)?;
    }
    // Ok(Response::default())
    Ok(messages)
    // Ok(Response::new().add_attribute("action", "custom_transfer").add_messages(messages))
}

pub fn _transfer_standard(
    storage: &mut dyn Storage,
    env: Env,
    sender: Addr,
    recipient: Addr,
    t_amount: Uint128
) -> StdResult<Uint128> {
    let (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity) = _get_values(storage, t_amount);
    
    let r1 = ROWNED.load(storage, sender.clone()).unwrap_or(Uint128::zero()) - r_amount;
    ROWNED.save(storage, sender.clone(), &r1)?;
    
    let r2 = ROWNED.load(storage, recipient.clone()).unwrap_or(Uint128::zero()) + r_transfer_amount;
    ROWNED.save(storage, recipient.clone(), &r2)?;

    _take_liquidity(storage, env, t_liquidity)?;
    _reflect_fee(storage, r_fee, t_fee)?;
    Ok(t_transfer_amount)
    
}

pub fn _transfer_to_excluded(
    storage: &mut dyn Storage,
    env: Env,
    sender: Addr,
    recipient: Addr,
    t_amount: Uint128
) -> StdResult<Uint128> {
    let (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity) = _get_values(storage, t_amount);

    let r1 = ROWNED.load(storage, sender.clone()).unwrap_or(Uint128::zero()) - r_amount;
    ROWNED.save(storage, sender.clone(), &r1)?;
    
    let t1 = TOWNED.load(storage, recipient.clone()).unwrap_or(Uint128::zero()) + t_transfer_amount;
    TOWNED.save(storage, recipient.clone(), &t1)?;
    
    let r2 = ROWNED.load(storage, recipient.clone()).unwrap_or(Uint128::zero()) + r_transfer_amount;
    ROWNED.save(storage, recipient.clone(), &r2)?;
    
    _take_liquidity(storage, env, t_liquidity)?;
    _reflect_fee(storage, r_fee, t_fee)?;
    Ok(t_transfer_amount)
    // _rOwned[sender] = _rOwned[sender].sub(r_amount);
    // _tOwned[recipient] = _tOwned[recipient].add(t_transfer_amount);
    // _rOwned[recipient] = _rOwned[recipient].add(r_transfer_amount);           
    // _takeLiquidity(t_liquidity);
    // _reflectFee(r_fee, t_fee);
    // emit Transfer(sender, recipient, t_transfer_amount);
}

pub fn _transfer_from_excluded(
    storage: &mut dyn Storage,
    env: Env,
    sender: Addr,
    recipient: Addr,
    t_amount: Uint128
) -> StdResult<Uint128> {

    let (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity) = _get_values(storage, t_amount);
    
    let t1 = TOWNED.load(storage, sender.clone()).unwrap_or(Uint128::zero()) - t_amount;
    TOWNED.save(storage, sender.clone(), &t1)?;
    
    let r1 = ROWNED.load(storage, sender.clone()).unwrap_or(Uint128::zero()) - r_amount;
    ROWNED.save(storage, sender.clone(), &r1)?;
    
    let r2 = ROWNED.load(storage, recipient.clone()).unwrap_or(Uint128::zero()) + r_transfer_amount;
    ROWNED.save(storage, recipient.clone(), &r2)?;
    
    _take_liquidity(storage, env, t_liquidity)?;
    _reflect_fee(storage, r_fee, t_fee)?;
    Ok(t_transfer_amount)
    // (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity) = _get_values(t_amount);
    // _tOwned[sender] = _tOwned[sender].sub(t_amount);
    // _rOwned[sender] = _rOwned[sender].sub(r_amount);
    // _rOwned[recipient] = _rOwned[recipient].add(r_transfer_amount);   
    // _takeLiquidity(t_liquidity);
    // _reflectFee(r_fee, t_fee);
    // emit Transfer(sender, recipient, t_transfer_amount);
}

pub fn _transfer_both_excluded(
    storage: &mut dyn Storage,
    env: Env,
    sender: Addr,
    recipient: Addr,
    t_amount: Uint128
) -> StdResult<Uint128> {
    let (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity) = _get_values(storage, t_amount);
    
    let t1 = TOWNED.load(storage, sender.clone()).unwrap_or(Uint128::zero()) - t_amount;
    TOWNED.save(storage, sender.clone(), &t1)?;
    
    let r1 = ROWNED.load(storage, sender.clone()).unwrap_or(Uint128::zero()) - r_amount;
    ROWNED.save(storage, sender.clone(), &r1)?;
    
    let t2 = TOWNED.load(storage, recipient.clone()).unwrap_or(Uint128::zero()) + t_transfer_amount;
    TOWNED.save(storage, recipient.clone(), &t2)?;
    
    let r2 = ROWNED.load(storage, recipient.clone()).unwrap_or(Uint128::zero()) + r_transfer_amount;
    ROWNED.save(storage, recipient.clone(), &r2)?;
    
    _take_liquidity(storage, env, t_liquidity)?;
    _reflect_fee(storage, r_fee, t_fee)?;
    Ok(t_transfer_amount)
    // (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity) = _get_values(t_amount);
    // _tOwned[sender] = _tOwned[sender].sub(t_amount);
    // _rOwned[sender] = _rOwned[sender].sub(r_amount);
    // _tOwned[recipient] = _tOwned[recipient].add(t_transfer_amount);
    // _rOwned[recipient] = _rOwned[recipient].add(r_transfer_amount);        
    // _takeLiquidity(t_liquidity);
    // _reflectFee(r_fee, t_fee);
    // emit Transfer(sender, recipient, t_transfer_amount);
}

pub fn execute_custom_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    amount: Uint128
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    let msgs = _transfer(deps.api, deps.storage, deps.querier, env, info.sender.clone(), rcpt_addr.clone(), amount, false)?;
    
    let res = Response::new()
        .add_messages(msgs)
        .add_attribute("action", "transfer")
        .add_attribute("from", info.sender)
        .add_attribute("to", recipient)
        .add_attribute("amount", amount);
    Ok(res)
}


pub fn execute_custom_transfer_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    recipient: String,
    amount: Uint128,
) -> Result<Response, ContractError> {
    let rcpt_addr = deps.api.addr_validate(&recipient)?;
    let owner_addr = deps.api.addr_validate(&owner)?;
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }
    // deduct allowance before doing anything else have enough allowance
    deduct_allowance(deps.storage, &owner_addr, &info.sender, &env.block, amount)?;
    _transfer(deps.api, deps.storage, deps.querier, env, owner_addr.clone(), rcpt_addr.clone(), amount, false)?;
    
    let res = Response::new().add_attributes(vec![
        attr("action", "transfer_from"),
        attr("from", owner),
        attr("to", recipient),
        attr("by", info.sender),
        attr("amount", amount),
    ]);
    Ok(res)
}


pub fn execute_custom_send(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    if amount == Uint128::zero() {
        return Err(ContractError::InvalidZeroAmount {});
    }

    let rcpt_addr = deps.api.addr_validate(&contract)?;

    let cfg = CONFIG.load(deps.storage)?;
    let flag;
    if rcpt_addr == cfg.pool_address {
        flag = true;
    } else {
        flag = false;
    }
    _transfer(deps.api, deps.storage, deps.querier, env, info.sender.clone(), rcpt_addr.clone(), amount, flag)?;
    // move the tokens to the contract
    
    let res = Response::new()
        .add_attribute("action", "send")
        .add_attribute("from", &info.sender)
        .add_attribute("to", &contract)
        .add_attribute("amount", amount)
        .add_message(
            Cw20ReceiveMsg {
                sender: info.sender.into(),
                amount,
                msg,
            }
            .into_cosmos_msg(contract)?,
        );
    Ok(res)
}


pub fn execute_custom_send_from(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    owner: String,
    contract: String,
    amount: Uint128,
    msg: Binary,
) -> Result<Response, ContractError> {
    let rcpt_addr = deps.api.addr_validate(&contract)?;
    let owner_addr = deps.api.addr_validate(&owner)?;

    // deduct allowance before doing anything else have enough allowance
    deduct_allowance(deps.storage, &owner_addr, &info.sender, &env.block, amount)?;

    let cfg = CONFIG.load(deps.storage)?;
    let flag;
    if rcpt_addr == cfg.pool_address {
        flag = true;
    } else {
        flag = false;
    }
    _transfer(deps.api, deps.storage, deps.querier, env, owner_addr.clone(), rcpt_addr.clone(), amount, flag)?;
    // move the tokens to the contract

    let attrs = vec![
        attr("action", "send_from"),
        attr("from", &owner),
        attr("to", &contract),
        attr("by", &info.sender),
        attr("amount", amount),
    ];

    // create a send message
    let msg = Cw20ReceiveMsg {
        sender: info.sender.into(),
        amount,
        msg,
    }
    .into_cosmos_msg(contract)?;

    let res = Response::new().add_message(msg).add_attributes(attrs);
    Ok(res)
}
fn map_excluded(
    item: StdResult<(Addr, bool)>,
) -> StdResult<ExcludedInfo> {
    item.map(|(addr, excluded)| {
        ExcludedInfo {
            addr,
            excluded
        }
    })
}
pub fn _get_rate(storage: &mut dyn Storage) -> Uint128 {
    let (r_supply, t_supply) = _get_current_supply(storage);
    return r_supply / t_supply;
}

pub fn _get_current_supply(storage: &mut dyn Storage) -> (Uint128, Uint128) {
    let cfg = CONFIG.load(storage).unwrap();
    let mut r_supply = cfg.r_total;
    let mut t_supply = cfg.t_total;

    let excluded_list:StdResult<Vec<_>> = ISEXCLUDED
        .range(storage, None, None, Order::Ascending)
        .map(|item| map_excluded(item))
        .collect();
    
    let mut r_val;
    let mut t_val;
    for item in excluded_list.unwrap() {
        if !item.excluded {
            continue;
        }
        r_val = ROWNED.load(storage, item.addr.clone()).unwrap_or(Uint128::zero());
        t_val = TOWNED.load(storage, item.addr.clone()).unwrap_or(Uint128::zero());
        if  r_val > r_supply ||  t_val > t_supply {
            return (cfg.r_total, cfg.t_total);
        }
        r_supply = r_supply - r_val;
        t_supply = t_supply - t_val;
        
    }
    if r_supply < cfg.r_total / cfg.t_total {
        return (cfg.r_total, cfg.t_total);
    }
    return (r_supply, t_supply);
    
}


pub fn _token_from_reflection(storage: &mut dyn Storage, r_amount:Uint128) -> Uint128 {
    let cfg = CONFIG.load(storage).unwrap();
    if r_amount > cfg.r_total {
        return Uint128::zero();
    }
    let current_rate = _get_rate(storage);
    return r_amount.checked_div(current_rate).unwrap();
}


#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        
        // inherited from cw20-base
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::TokenInfo {} => to_binary(&custom_query_token_info(deps)?),
        QueryMsg::Balance { address } => to_binary(&custom_query_balance(deps, address)?),
        QueryMsg::Allowance { owner, spender } => {
            to_binary(&query_allowance(deps, owner, spender)?)
        },
        QueryMsg::IsExcludedFromReward { address } => {
            to_binary(&query_is_excluded_from_reward(deps, address)?)
        },
        QueryMsg::TotalFees { } => {
            to_binary(&query_total_fees(deps)?)
        },
        QueryMsg::ReflectionFromToken { t_amount, deduct_transfer_fee } => {
            to_binary(&query_reflection_from_token(deps, t_amount, deduct_transfer_fee)?)
        },
        QueryMsg::TokenFromReflection { r_amount } => {
            to_binary(&query_token_from_reflection(deps, r_amount)?)
        },
        QueryMsg::IsExcludedFromFee { address } => {
            to_binary(&query_is_excluded_from_fee(deps, address)?)
        },
        
    }
}

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let cfg = CONFIG.load(deps.storage)?;
    Ok(ConfigResponse {
        owner: cfg.owner,
        t_total: cfg.t_total,
        r_total: cfg.r_total,
        t_fee_total: cfg.t_fee_total,
        tax_fee: cfg.tax_fee,
        previous_tax_fee: cfg.previous_tax_fee,
        liquidity_fee: cfg.liquidity_fee,
        previous_liquidity_fee: cfg.previous_liquidity_fee,
        pool_address: cfg.pool_address,
        swap_and_liquidity_enabled: cfg.swap_and_liquidity_enabled,
        max_tx_amount: cfg.max_tx_amount,
        num_tokens_sell_to_add_to_liquidity: cfg.num_tokens_sell_to_add_to_liquidity
    })
}
pub fn custom_query_token_info(deps: Deps) -> StdResult<TokenInfoResponse> {
    let info = TOKEN_INFO.load(deps.storage)?;
    let cfg = CONFIG.load(deps.storage)?;
    let res = TokenInfoResponse {
        name: info.name,
        symbol: info.symbol,
        decimals: info.decimals,
        total_supply: cfg.t_total,
    };
    Ok(res)
}

pub fn custom_query_balance(deps: Deps, address: String) -> StdResult<CW20BalanceResponse> {
    let address = deps.api.addr_validate(&address)?;
    let mut balance ;
    if ISEXCLUDED.load(deps.storage, address.clone()).unwrap_or(false) {
        balance = TOWNED.load(deps.storage, address.clone()).unwrap_or(Uint128::zero());
    } else {
        balance = _token_from_reflection_immut(deps.storage, ROWNED.load(deps.storage, address.clone()).unwrap_or(Uint128::zero()));
    }
    
    Ok(CW20BalanceResponse { balance })
}

pub fn query_is_excluded_from_reward(deps: Deps, address: Addr) -> StdResult<BoolResponse> {
    let ret = ISEXCLUDED.load(deps.storage, address.clone()).unwrap_or(false);
    Ok(BoolResponse { ret })
}

pub fn query_is_excluded_from_fee(deps: Deps, address: Addr) -> StdResult<BoolResponse> {
    let ret = ISEXCLUDEDFROMFEE.load(deps.storage, address.clone()).unwrap_or(false);
    Ok(BoolResponse { ret })
}

pub fn query_total_fees(deps: Deps) -> StdResult<Uint128Response> {
    let cfg = CONFIG.load(deps.storage)?;
    let ret = cfg.t_fee_total;
    Ok(Uint128Response { ret })
}

pub fn query_reflection_from_token(deps: Deps, t_amount: Uint128, deduct_transfer_fee: bool) -> StdResult<Uint128Response> {
    // let mut ret = Uint128::zero();
    
    // let cfg = CONFIG.load(deps.storage)?;

    // // if (t_amount <= cfg.t_total) {
    // //     if !deduct_transfer_fee {
    // //         let (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity) = _get_values(deps.storage , t_amount);
    // //     }
    // // }
    // let ret = cfg.t_fee_total;
    Ok(Uint128Response { ret: Uint128::zero() })
}


pub fn query_token_from_reflection(deps: Deps, r_amount: Uint128) -> StdResult<Uint128Response> {
    // let mut ret = Uint128::zero();
    
    // let cfg = CONFIG.load(deps.storage)?;

    // // if (t_amount <= cfg.t_total) {
    // //     if !deduct_transfer_fee {
    // //         let (r_amount, r_transfer_amount, r_fee, t_transfer_amount, t_fee, t_liquidity) = _get_values(deps.storage , t_amount);
    // //     }
    // // }
    // let ret = cfg.t_fee_total;
    Ok(Uint128Response { ret: Uint128::zero() })
}

pub fn _token_from_reflection_immut(storage: &dyn Storage, r_amount:Uint128) -> Uint128 {
    let cfg = CONFIG.load(storage).unwrap();
    if r_amount > cfg.r_total {
        return Uint128::zero();
    }
    let current_rate = _get_rate_immut(storage);
    return r_amount.checked_div(current_rate).unwrap();
}

pub fn _get_rate_immut(storage: &dyn Storage) -> Uint128 {
    let (r_supply, t_supply) = _get_current_supply_immut(storage);
    return r_supply / t_supply;
}


pub fn _get_current_supply_immut(storage: &dyn Storage) -> (Uint128, Uint128) {
    let cfg = CONFIG.load(storage).unwrap();
    let mut r_supply = cfg.r_total;
    let mut t_supply = cfg.t_total;

    let excluded_list:StdResult<Vec<_>> = ISEXCLUDED
        .range(storage, None, None, Order::Ascending)
        .map(|item| map_excluded(item))
        .collect();
    
    let mut r_val;
    let mut t_val;
    for item in excluded_list.unwrap() {
        if !item.excluded {
            continue;
        }
        r_val = ROWNED.load(storage, item.addr.clone()).unwrap_or(Uint128::zero());
        t_val = TOWNED.load(storage, item.addr.clone()).unwrap_or(Uint128::zero());
        if  r_val > r_supply ||  t_val > t_supply {
            return (cfg.r_total, cfg.t_total);
        }
        r_supply = r_supply - r_val;
        t_supply = t_supply - t_val;
        
    }
    if r_supply < cfg.r_total / cfg.t_total {
        return (cfg.r_total, cfg.t_total);
    }
    return (r_supply, t_supply);
    
}