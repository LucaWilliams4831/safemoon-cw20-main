use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Decimal, Uint128};
use cw_controllers::Claims;
use cw_utils::Duration;
use cw_storage_plus::{Item, Map};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Config {
    /// Owner If None set, contract is frozen.
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
    pub num_tokens_sell_to_add_to_liquidity: Uint128,
    pub denom: String
}
pub const CONFIG: Item<Config> = Item::new("config");


pub const ROWNED: Map<Addr, Uint128> = Map::new("_rOwned");
pub const TOWNED: Map<Addr, Uint128> = Map::new("_tOwned");
pub const ISEXCLUDEDFROMFEE: Map<Addr, bool> = Map::new("isExcludedFromFee");
pub const ISEXCLUDED: Map<Addr, bool> = Map::new("isExcluded");

