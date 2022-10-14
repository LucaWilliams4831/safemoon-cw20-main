use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Binary, Coin, Decimal, Uint128, Addr};
use cw20::Expiration;
pub use cw_controllers::ClaimsResponse;
use cw_utils::Duration;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// name of the derivative token
    pub name: String,
    /// symbol / ticker of the derivative token
    pub symbol: String,
    /// decimal places of the derivative token (for UI)
    pub decimals: u8,

    /// tax_fee: 5
    pub tax_fee: u8,
    /// liquidity_fee: 5
    pub liquidity_fee: u8,
    pub pool_address: Addr,
    pub denom: String
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    UpdateConfig { new_owner:String },
    UpdatePool { address:Addr },
    /// Implements CW20. Transfer is a base message to move tokens to another account without triggering actions
    Transfer { recipient: String, amount: Uint128 },
    Send {
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    /// Implements CW20 "approval" extension. Allows spender to access an additional amount tokens
    /// from the owner's (env.sender) account. If expires is Some(), overwrites current allowance
    /// expiration with this one.
    IncreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    /// Implements CW20 "approval" extension. Lowers the spender's access of tokens
    /// from the owner's (env.sender) account by amount. If expires is Some(), overwrites current
    /// allowance expiration with this one.
    DecreaseAllowance {
        spender: String,
        amount: Uint128,
        expires: Option<Expiration>,
    },
    /// Implements CW20 "approval" extension. Transfers amount tokens from owner -> recipient
    /// if `env.sender` has sufficient pre-approval.
    TransferFrom {
        owner: String,
        recipient: String,
        amount: Uint128,
    },
    /// Implements CW20 "approval" extension. Sends amount tokens from owner -> contract
    /// if `env.sender` has sufficient pre-approval.
    SendFrom {
        owner: String,
        contract: String,
        amount: Uint128,
        msg: Binary,
    },
    
    /// Custom Execute from safemoon
    
    Deliver {
        amount: Uint128
    },
    ExcludeFromReward {
        address: Addr
    },
    IncludeInReward {
        address: Addr
    },
    ExcludeFromFee {
        address: Addr
    },
    IncludeInFee {
        address: Addr
    },
    SetTaxFeePercent {
        percent: u8
    },
    SetLiquidityFeePercent {
        percent: u8
    },
    SetMaxTxPercent {
        percent: u8
    },
    SetSwapAndLiquifyEnabled {
        enabled: bool
    },
    FetchAdmin {}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    /// Implements CW20. Returns the current balance of the given address, 0 if unset.
    Balance { address: String },
    /// Implements CW20. Returns metadata on the contract - name, decimals, supply, etc.
    TokenInfo {},
    /// Implements CW20 "allowance" extension.
    /// Returns how much spender can use from owner account, 0 if unset.
    Allowance { owner: String, spender: String },

    IsExcludedFromReward {address: Addr},
    TotalFees {},
    ReflectionFromToken {t_amount: Uint128, deduct_transfer_fee: bool},
    TokenFromReflection {r_amount: Uint128},
    IsExcludedFromFee {address: Addr}

}
#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct ExcludedInfo {
    pub addr: Addr,
    pub excluded: bool
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct Uint128Response {
    pub ret: Uint128,
}


#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct BoolResponse {
    pub ret: bool
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
#[serde(rename_all = "snake_case")]
pub struct ConfigResponse {
    pub owner: Addr,
    pub t_total: Uint128,
    pub r_total: Uint128,
    pub t_fee_total: Uint128,
    pub tax_fee: u8,
    pub previous_tax_fee: u8,
    pub liquidity_fee: u8,
    pub previous_liquidity_fee: u8,
    pub pool_address: Addr,
    pub swap_and_liquidity_enabled: bool,
    pub max_tx_amount: Uint128,
    pub num_tokens_sell_to_add_to_liquidity: Uint128

}
