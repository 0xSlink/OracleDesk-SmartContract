#![no_std]

use soroban_sdk::{
    auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation},
    contract, contracterror, contractevent, contractimpl, contracttype, token, vec, Address,
    BytesN, Env, IntoVal, String, Symbol, Vec,
};

// See the comment on the equivalent `mod market_core` in
// contracts/resolver/src/lib.rs: a Cargo path dependency on market-core
// breaks the wasm build (duplicate `__constructor` export), so its client is
// generated from the compiled wasm instead. `docs/adr/0002-cross-contract-calls.md`.
#[allow(clippy::too_many_arguments)]
mod market_core {
    soroban_sdk::contractimport!(file = "market_core.wasm");
}
use market_core::{Category, Client as MarketCoreClient, MarketStatus, Outcome};

const DAY_SECONDS: u64 = 86_400;
const BUMP_THRESHOLD: u32 = 30 * 17_280;
const BUMP_TO: u32 = 60 * 17_280;
/// Width of one bucket of the sliding daily window.
const BUCKET_SECONDS: u64 = 3_600;
/// Buckets checked against `max_daily`: the trade's own hour plus the 24
/// before it. Any real 24h interval `(t - 24h, t]` touches at most hours
/// `h - 24 ..= h` (`h = t / 3600`), so summing 25 buckets never under-counts;
/// with 24 the partially-covered hour `h - 24` could add up to one more
/// bucket's worth on top of `max_daily`. The effective window is therefore
/// 24-25h: slightly conservative, never permissive.
const BUCKETS: u32 = 25;
/// Basis points in 1 (prices from market-core are in bps).
const BPS: i128 = 10_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiskConfig {
    pub agent: Address,
    pub collateral: Address,
    pub market_core: Address,
    pub max_trade: i128,
    pub max_market: i128,
    pub max_daily: i128,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketExposure {
    pub amount: i128,
    pub open: bool,
}

/// Summary of the sliding daily window, returned by `daily_usage`.
/// `day_start` is the start of the oldest hour still counted and `amount`
/// the usage summed over the whole window (see [`UsageWindow`]).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DailyUsage {
    pub day_start: u64,
    pub amount: i128,
}

/// Hourly ring buffer behind `max_daily`. Kept in instance storage: ~400
/// bytes that every call loads anyway, so it costs no extra ledger-entry
/// read (a persistent entry would add one per trade).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UsageWindow {
    /// Hour index (`timestamp / BUCKET_SECONDS`) of the newest bucket.
    pub head_hour: u64,
    /// Always `BUCKETS` long. `amounts[i]` is the usage of hour
    /// `head_hour - ((head_idx - i) mod BUCKETS)`.
    pub amounts: Vec<i128>,
    pub head_idx: u32,
}

/// Collateral the treasury has sent into (`collateral_in`) and received back
/// from (`collateral_out`) one market. Accounting starts when this code was
/// deployed: a missing entry reads as zero.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NetPosition {
    pub collateral_in: i128,
    pub collateral_out: i128,
}

impl NetPosition {
    /// Capital still deployed: `max(0, in - out)`. Profit floors at zero so it
    /// never offsets exposure in another market.
    fn cost_basis(&self) -> i128 {
        self.collateral_in
            .saturating_sub(self.collateral_out)
            .max(0)
    }
}

/// `market_valuation` result. Only `cost_basis` is enforced (by
/// `max_net_exposure`); the others are informational.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Valuation {
    pub cost_basis: i128,
    /// What the treasury gets back whatever the outcome, from state a price
    /// move cannot raise: complete sets `min(yes, no)` plus, as the unclaimed
    /// LP, `min(reserve_yes, reserve_no)`. After finality, the settlement
    /// value.
    pub floor_value: i128,
    /// Holdings at the pool's spot price. Manipulable by any trader, so it is
    /// never used for enforcement.
    pub spot_value: i128,
    /// `max(0, cost_basis - floor_value)`.
    pub at_risk: i128,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Config,
    Market(u64),
    /// Legacy fixed-window `DailyUsage`; migrated into `UsageWindow` and
    /// removed on the first trade after an upgrade.
    Daily,
    Paused,
    Successor,
    UsageWindow,
    /// `max_net_exposure` (kept out of `RiskConfig` so its XDR shape and
    /// `get_config` stay binding-compatible).
    NetCap,
    Net(u64),
    /// Running `sum(max(0, in - out))` over all markets.
    NetTotal,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    Unauthorized = 2,
    InvalidConfig = 3,
    InvalidAmount = 4,
    TradeCapExceeded = 5,
    MarketCapExceeded = 6,
    DailyCapExceeded = 7,
    MarketNotOpen = 8,
    InsufficientBalance = 9,
    Paused = 10,
    NoSuccessor = 11,
    /// Not market-core's error 12 (`SlippageExceeded`): market-core errors
    /// reach callers of treasury entrypoints with market-core's own codes.
    NetExposureExceeded = 12,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeAuthorized {
    #[topic]
    pub market_id: u64,
    #[topic]
    pub agent: Address,
    pub amount: i128,
    pub market_total: i128,
    pub daily_total: i128,
}

/// Net exposure after any treasury money movement in `market_id`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetExposureUpdated {
    #[topic]
    pub market_id: u64,
    pub net_market: i128,
    pub net_total: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PositionClosed {
    #[topic]
    pub market_id: u64,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeesCollected {
    #[topic]
    pub market_id: u64,
    pub amount: i128,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuccessorSet {
    pub old: Option<Address>,
    pub new: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarketMigrated {
    #[topic]
    pub market_id: u64,
    #[topic]
    pub successor: Address,
    pub yes: i128,
    pub no: i128,
    pub lp: bool,
}

#[contract]
pub struct Treasury;

#[contractimpl]
impl Treasury {
    /// `max_net_exposure` caps capital deployed in open positions across all
    /// markets at once (see `net_exposure`); it must be at least `max_trade`.
    #[allow(clippy::too_many_arguments)]
    pub fn __constructor(
        env: Env,
        admin: Address,
        agent: Address,
        collateral: Address,
        market_core: Address,
        max_trade: i128,
        max_market: i128,
        max_daily: i128,
        max_net_exposure: i128,
    ) {
        if max_trade <= 0
            || max_market < max_trade
            || max_daily < max_market
            || max_net_exposure < max_trade
        {
            panic!("invalid risk configuration");
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(
            &DataKey::Config,
            &RiskConfig {
                agent,
                collateral,
                market_core,
                max_trade,
                max_market,
                max_daily,
            },
        );
        env.storage().instance().set(
            &DataKey::UsageWindow,
            &Self::empty_window(&env, env.ledger().timestamp() / BUCKET_SECONDS),
        );
        env.storage()
            .instance()
            .set(&DataKey::NetCap, &max_net_exposure);
        env.storage().instance().set(&DataKey::Paused, &false);
    }

    pub fn get_config(env: Env) -> Result<RiskConfig, Error> {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(Error::NotInitialized)
    }

    pub fn available_capital(env: Env) -> Result<i128, Error> {
        let config = Self::get_config(env.clone())?;
        Ok(token::TokenClient::new(&env, &config.collateral)
            .balance(&env.current_contract_address()))
    }

    /// Usage over the sliding window as of now: `amount` sums every bucket
    /// still counted against `max_daily`, `day_start` is the start of the
    /// oldest of them. Side-effect free (it advances a copy), so repeated
    /// reads without trades in between return the same value.
    pub fn daily_usage(env: Env) -> Result<DailyUsage, Error> {
        let window = Self::daily_usage_detail(env)?;
        Ok(DailyUsage {
            day_start: window
                .head_hour
                .saturating_sub((BUCKETS - 1) as u64)
                .saturating_mul(BUCKET_SECONDS),
            amount: Self::window_sum(&window)?,
        })
    }

    /// The hourly buckets behind `daily_usage`, advanced to now (not stored).
    pub fn daily_usage_detail(env: Env) -> Result<UsageWindow, Error> {
        let mut window = Self::load_window(&env)?;
        Self::advance(&mut window, env.ledger().timestamp() / BUCKET_SECONDS);
        Ok(window)
    }

    /// How much more the agent may deploy right now before `max_daily`
    /// (`max_daily - window sum`, floored at 0), for pre-flight checks.
    pub fn daily_headroom(env: Env) -> Result<i128, Error> {
        let config = Self::get_config(env.clone())?;
        let used = Self::daily_usage(env)?.amount;
        Ok(config.max_daily.saturating_sub(used).max(0))
    }

    /// `max_net_exposure`. A treasury deployed before the net cap existed has
    /// no stored value and uses `max_daily`.
    pub fn get_net_cap(env: Env) -> Result<i128, Error> {
        let config = Self::get_config(env.clone())?;
        Ok(env
            .storage()
            .instance()
            .get(&DataKey::NetCap)
            .unwrap_or(config.max_daily))
    }

    /// Capital currently deployed across all markets:
    /// `sum(max(0, collateral_in - collateral_out))`, the figure
    /// `max_net_exposure` is enforced against.
    pub fn net_exposure(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::NetTotal)
            .unwrap_or(0)
    }

    pub fn net_position(env: Env, market_id: u64) -> NetPosition {
        Self::load_net(&env, market_id)
    }

    /// Conservative valuation of the treasury's holdings in `market_id`
    /// (see [`Valuation`]). Two cross-contract reads (`get_market`,
    /// `get_position`); a view only, never on the trading path.
    pub fn market_valuation(env: Env, market_id: u64) -> Result<Valuation, Error> {
        let config = Self::get_config(env.clone())?;
        let treasury_addr = env.current_contract_address();
        let client = MarketCoreClient::new(&env, &config.market_core);
        let market = client.get_market(&market_id);
        let position = client.get_position(&market_id, &treasury_addr);
        let lp = market.creator == treasury_addr && !market.lp_claimed;
        let (lp_yes, lp_no) = if lp {
            (market.reserve_yes, market.reserve_no)
        } else {
            (0, 0)
        };

        let (floor_value, spot_value) = match market.status {
            MarketStatus::Open => {
                let floor = position
                    .yes
                    .min(position.no)
                    .saturating_add(lp_yes.min(lp_no));
                // Mirrors market-core's `price_yes_bps` from the reserves just
                // read, so spot costs no third call.
                let sum = market.reserve_yes.saturating_add(market.reserve_no);
                let p_yes = if sum > 0 {
                    market.reserve_no.saturating_mul(BPS) / sum
                } else {
                    BPS / 2
                };
                let yes = position.yes.saturating_add(lp_yes);
                let no = position.no.saturating_add(lp_no);
                let spot = yes
                    .saturating_mul(p_yes)
                    .saturating_add(no.saturating_mul(BPS - p_yes))
                    / BPS;
                (floor, spot)
            }
            // Final markets are worth exactly what market-core's
            // `settle_value` pays out.
            status => {
                let settled = Self::settle_value(&status, position.yes, position.no)
                    .saturating_add(Self::settle_value(&status, lp_yes, lp_no));
                (settled, settled)
            }
        };
        let cost_basis = Self::load_net(&env, market_id).cost_basis();
        Ok(Valuation {
            cost_basis,
            floor_value,
            spot_value,
            at_risk: cost_basis.saturating_sub(floor_value).max(0),
        })
    }

    pub fn set_paused(env: Env, paused: bool) -> Result<(), Error> {
        Self::require_admin(&env)?;
        env.storage().instance().set(&DataKey::Paused, &paused);
        Ok(())
    }

    pub fn deposit(env: Env, from: Address, amount: i128) -> Result<(), Error> {
        let config = Self::get_config(env.clone())?;
        from.require_auth();
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        token::TokenClient::new(&env, &config.collateral).transfer(
            &from,
            env.current_contract_address(),
            &amount,
        );
        Ok(())
    }

    pub fn withdraw(env: Env, to: Address, amount: i128) -> Result<(), Error> {
        Self::require_admin(&env)?;
        let config = Self::get_config(env.clone())?;
        if amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        let token = token::TokenClient::new(&env, &config.collateral);
        if token.balance(&env.current_contract_address()) < amount {
            return Err(Error::InsufficientBalance);
        }
        token.transfer(&env.current_contract_address(), &to, &amount);
        Ok(())
    }

    // ----- agent-driven trading --------------------------------------------
    // The agent never touches market-core directly with treasury funds: it
    // asks the treasury to do so, the treasury checks its own on-chain caps,
    // then calls market-core itself as the trader/creator. This is the only
    // path that moves treasury capital, so the caps below are real limits on
    // what the automated agent can deploy, not just off-chain bookkeeping.

    /// Creates and seeds a market using treasury capital. The treasury
    /// becomes the market's creator/LP.
    #[allow(clippy::too_many_arguments)]
    pub fn agent_create_market(
        env: Env,
        question_hash: BytesN<32>,
        resolution_hash: BytesN<32>,
        meta_uri: String,
        category: Category,
        close_time: u64,
        seed_amount: i128,
        initial_yes_bps: u32,
    ) -> Result<u64, Error> {
        Self::assert_not_paused(&env)?;
        let config = Self::get_config(env.clone())?;
        config.agent.require_auth();
        if seed_amount <= 0 {
            return Err(Error::InvalidAmount);
        }
        if seed_amount > config.max_trade {
            return Err(Error::TradeCapExceeded);
        }

        let treasury_addr = env.current_contract_address();
        Self::authorize_token_pull(&env, &config, &treasury_addr, seed_amount);
        let market_id = MarketCoreClient::new(&env, &config.market_core).create_market(
            &treasury_addr,
            &question_hash,
            &resolution_hash,
            &meta_uri,
            &category,
            &close_time,
            &seed_amount,
            &initial_yes_bps,
        );
        Self::reserve_capacity(&env, &config, market_id, seed_amount)?;
        Self::reserve_net(&env, market_id, seed_amount)?;
        Ok(market_id)
    }

    /// Buys `outcome` shares in an existing market using treasury capital.
    pub fn agent_buy(
        env: Env,
        market_id: u64,
        outcome: Outcome,
        collateral_in: i128,
        min_shares_out: i128,
    ) -> Result<i128, Error> {
        Self::do_agent_buy(env, market_id, outcome, collateral_in, min_shares_out, None)
    }

    /// `agent_buy` with a per-share limit price (`limit_price_bps`, 0 = none)
    /// and a deadline (unix seconds, network time). market-core's `buy_v2`
    /// enforces both; an expired or out-of-limit trade reverts the whole
    /// invocation, so the treasury's caps are never consumed by a reverted
    /// trade.
    pub fn agent_buy_v2(
        env: Env,
        market_id: u64,
        outcome: Outcome,
        collateral_in: i128,
        min_shares_out: i128,
        limit_price_bps: u32,
        deadline: u64,
    ) -> Result<i128, Error> {
        Self::do_agent_buy(
            env,
            market_id,
            outcome,
            collateral_in,
            min_shares_out,
            Some((limit_price_bps, deadline)),
        )
    }

    fn do_agent_buy(
        env: Env,
        market_id: u64,
        outcome: Outcome,
        collateral_in: i128,
        min_shares_out: i128,
        bound: Option<(u32, u64)>,
    ) -> Result<i128, Error> {
        Self::assert_not_paused(&env)?;
        let config = Self::get_config(env.clone())?;
        config.agent.require_auth();
        if collateral_in <= 0 {
            return Err(Error::InvalidAmount);
        }
        if collateral_in > config.max_trade {
            return Err(Error::TradeCapExceeded);
        }

        let treasury_addr = env.current_contract_address();
        // market-core's `buy` pulls `collateral_in` from `trader` (here, the
        // treasury itself) via `token.transfer(trader -> market-core)`. That
        // transfer is a call the *token* contract receives from market-core,
        // not from the treasury directly, so the treasury's implicit
        // self-authorization (as the direct caller of market-core) does not
        // cover it. Without this pre-authorization the nested transfer's
        // `from.require_auth()` has no matching entry and the whole call
        // reverts with an authorization error — see
        // `pitfall_agent_buy_fails_without_nested_transfer_preauth` below and
        // `docs/adr/0002-treasury-nested-auth.md`.
        Self::authorize_token_pull(&env, &config, &treasury_addr, collateral_in);
        let client = MarketCoreClient::new(&env, &config.market_core);
        let shares = match bound {
            Some((limit_price_bps, deadline)) => client.buy_v2(
                &treasury_addr,
                &market_id,
                &outcome,
                &collateral_in,
                &min_shares_out,
                &limit_price_bps,
                &deadline,
            ),
            None => client.buy(
                &treasury_addr,
                &market_id,
                &outcome,
                &collateral_in,
                &min_shares_out,
            ),
        };
        Self::reserve_capacity(&env, &config, market_id, collateral_in)?;
        Self::reserve_net(&env, market_id, collateral_in)?;
        Ok(shares)
    }

    /// Sells `outcome` shares back into the pool for exactly
    /// `collateral_out`. This returns capital to the treasury (market-core
    /// pays out of its own balance), so it is not capped and does not need
    /// the nested-transfer pre-authorization that buying does. It frees net
    /// headroom (`collateral_out` is recorded) but never lifetime headroom.
    pub fn agent_sell(
        env: Env,
        market_id: u64,
        outcome: Outcome,
        collateral_out: i128,
        max_shares_in: i128,
    ) -> Result<i128, Error> {
        Self::do_agent_sell(env, market_id, outcome, collateral_out, max_shares_in, None)
    }

    /// `agent_sell` with a per-share limit price (`limit_price_bps`, 0 = no
    /// limit) and a deadline (unix seconds, network time), both enforced by
    /// market-core's `sell_v2`.
    pub fn agent_sell_v2(
        env: Env,
        market_id: u64,
        outcome: Outcome,
        collateral_out: i128,
        max_shares_in: i128,
        limit_price_bps: u32,
        deadline: u64,
    ) -> Result<i128, Error> {
        Self::do_agent_sell(
            env,
            market_id,
            outcome,
            collateral_out,
            max_shares_in,
            Some((limit_price_bps, deadline)),
        )
    }

    fn do_agent_sell(
        env: Env,
        market_id: u64,
        outcome: Outcome,
        collateral_out: i128,
        max_shares_in: i128,
        bound: Option<(u32, u64)>,
    ) -> Result<i128, Error> {
        Self::assert_not_paused(&env)?;
        let config = Self::get_config(env.clone())?;
        config.agent.require_auth();
        let treasury_addr = env.current_contract_address();
        let client = MarketCoreClient::new(&env, &config.market_core);
        let shares_in = match bound {
            Some((limit_price_bps, deadline)) => client.sell_v2(
                &treasury_addr,
                &market_id,
                &outcome,
                &collateral_out,
                &max_shares_in,
                &limit_price_bps,
                &deadline,
            ),
            None => client.sell(
                &treasury_addr,
                &market_id,
                &outcome,
                &collateral_out,
                &max_shares_in,
            ),
        };
        // market-core transfers exactly `collateral_out` on success.
        Self::record_return(&env, market_id, collateral_out)?;
        Ok(shares_in)
    }

    /// Marks a market's exposure as exited. Pure bookkeeping (see
    /// `MarketExposure`) for the future exposure dashboard; it does not move
    /// funds or free either the cumulative per-market cap or net headroom.
    pub fn close_trade(env: Env, market_id: u64) -> Result<i128, Error> {
        let config = Self::get_config(env.clone())?;
        config.agent.require_auth();
        let mut exposure = Self::market_exposure(&env, market_id);
        if !exposure.open {
            return Err(Error::MarketNotOpen);
        }
        exposure.open = false;
        let amount = exposure.amount;
        Self::save_market(&env, market_id, &exposure);
        PositionClosed { market_id, amount }.publish(&env);
        Ok(amount)
    }

    /// Redeems the treasury's position in a resolved/voided market.
    pub fn redeem(env: Env, market_id: u64) -> Result<i128, Error> {
        let config = Self::get_config(env.clone())?;
        config.agent.require_auth();
        let treasury_addr = env.current_contract_address();
        let payout =
            MarketCoreClient::new(&env, &config.market_core).redeem(&treasury_addr, &market_id);
        Self::record_return(&env, market_id, payout)?;
        Ok(payout)
    }

    /// Claims the LP pool remainder for a market the treasury created.
    pub fn claim_pool_remainder(env: Env, market_id: u64) -> Result<i128, Error> {
        let config = Self::get_config(env.clone())?;
        config.agent.require_auth();
        let payout =
            MarketCoreClient::new(&env, &config.market_core).claim_pool_remainder(&market_id);
        Self::record_return(&env, market_id, payout)?;
        Ok(payout)
    }

    /// Claims the market's LP fee share for a market the treasury created.
    /// The LP portion accrues separately on market-core (see its `claim_lp_fees`);
    /// this is that claim, agent-gated like the other trading entrypoints.
    pub fn agent_claim_lp_fees(env: Env, market_id: u64) -> Result<i128, Error> {
        let config = Self::get_config(env.clone())?;
        config.agent.require_auth();
        let amount = MarketCoreClient::new(&env, &config.market_core).claim_lp_fees(&market_id);
        Self::record_return(&env, market_id, amount)?;
        Ok(amount)
    }

    /// Pulls accrued **protocol** fees from a configured market-core contract.
    ///
    /// Only the protocol's share of each market's fees: the LP portion accrues
    /// separately on market-core and belongs to the market's creator — for
    /// agent-created markets that is the treasury itself, via
    /// `agent_claim_lp_fees`.
    pub fn collect_market_fees(env: Env, market_id: u64) -> Result<i128, Error> {
        Self::require_admin(&env)?;
        let config = Self::get_config(env.clone())?;
        let amount = MarketCoreClient::new(&env, &config.market_core).sweep_fees(&market_id);
        FeesCollected { market_id, amount }.publish(&env);
        Ok(amount)
    }

    // ----- migration ---------------------------------------------------------

    /// Sets the successor treasury that `migrate_market` hands positions and
    /// the LP role to. Admin-only; overwrites any previous successor (the
    /// `SuccessorSet` events are the audit trail).
    pub fn set_successor(env: Env, successor: Address) -> Result<(), Error> {
        Self::require_admin(&env)?;
        let old: Option<Address> = env.storage().instance().get(&DataKey::Successor);
        env.storage()
            .instance()
            .set(&DataKey::Successor, &successor);
        SuccessorSet {
            old,
            new: successor,
        }
        .publish(&env);
        Ok(())
    }

    /// Moves the treasury's position in `market_id` — and the LP role, when
    /// the treasury is the market's creator and has not claimed the remainder
    /// yet — to the successor set via `set_successor`, then closes the local
    /// exposure. Admin-only: relocating capital is not an agent trading
    /// action, and the agent key must not be able to drain positions.
    ///
    /// market-core's `transfer_position` / `transfer_lp` require the treasury
    /// itself to authorize. Because the treasury is their *direct* caller
    /// here, the implicit invoker authorization covers both — no
    /// `authorize_as_current_contract` entry is needed (unlike the nested
    /// token transfer in `agent_buy`, see `docs/adr/0002-treasury-nested-auth.md`).
    pub fn migrate_market(env: Env, market_id: u64) -> Result<(), Error> {
        Self::require_admin(&env)?;
        let successor: Address = env
            .storage()
            .instance()
            .get(&DataKey::Successor)
            .ok_or(Error::NoSuccessor)?;
        let config = Self::get_config(env.clone())?;
        let treasury_addr = env.current_contract_address();
        let client = MarketCoreClient::new(&env, &config.market_core);

        let position = client.get_position(&market_id, &treasury_addr);
        if position.yes > 0 {
            client.transfer_position(
                &treasury_addr,
                &successor,
                &market_id,
                &Outcome::Yes,
                &position.yes,
            );
        }
        if position.no > 0 {
            client.transfer_position(
                &treasury_addr,
                &successor,
                &market_id,
                &Outcome::No,
                &position.no,
            );
        }
        let market = client.get_market(&market_id);
        let lp = market.creator == treasury_addr && !market.lp_claimed;
        if lp {
            client.transfer_lp(&market_id, &successor);
        }
        let mut exposure = Self::market_exposure(&env, market_id);
        exposure.open = false;
        Self::save_market(&env, market_id, &exposure);
        // The holdings now belong to the successor, so none of this market's
        // capital is at risk here any more.
        Self::clear_net(&env, market_id)?;
        MarketMigrated {
            market_id,
            successor,
            yes: position.yes,
            no: position.no,
            lp,
        }
        .publish(&env);
        Ok(())
    }
}

impl Treasury {
    fn require_admin(env: &Env) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }

    fn assert_not_paused(env: &Env) -> Result<(), Error> {
        let paused: bool = env
            .storage()
            .instance()
            .get(&DataKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(Error::Paused);
        }
        Ok(())
    }

    fn empty_window(env: &Env, head_hour: u64) -> UsageWindow {
        let mut amounts = Vec::new(env);
        for _ in 0..BUCKETS {
            amounts.push_back(0i128);
        }
        UsageWindow {
            head_hour,
            amounts,
            head_idx: 0,
        }
    }

    /// The stored window, or one migrated from the legacy fixed-window
    /// `DataKey::Daily` (not persisted here; `reserve_capacity` saves it and
    /// removes the legacy entry). Legacy usage still inside its day counts in
    /// full in the current hour's bucket: conservative, since it then stays
    /// in the window for up to 25 more hours.
    fn load_window(env: &Env) -> Result<UsageWindow, Error> {
        let storage = env.storage().instance();
        if let Some(window) = storage.get(&DataKey::UsageWindow) {
            return Ok(window);
        }
        let legacy: DailyUsage = storage.get(&DataKey::Daily).ok_or(Error::NotInitialized)?;
        let now = env.ledger().timestamp();
        let mut window = Self::empty_window(env, now / BUCKET_SECONDS);
        if now < legacy.day_start.saturating_add(DAY_SECONDS) {
            window.amounts.set(window.head_idx, legacy.amount);
        }
        Ok(window)
    }

    /// Moves the head to `now_hour`, zeroing every bucket that left the
    /// window. At most `BUCKETS` steps however long the treasury was idle.
    /// An equal or earlier hour is a no-op: ledger time never goes backwards
    /// on-chain (tests that rewind `set_timestamp` just keep the current head).
    fn advance(window: &mut UsageWindow, now_hour: u64) {
        if now_hour <= window.head_hour {
            return;
        }
        let steps = core::cmp::min(now_hour - window.head_hour, BUCKETS as u64) as u32;
        for _ in 0..steps {
            window.head_idx = (window.head_idx + 1) % BUCKETS;
            window.amounts.set(window.head_idx, 0);
        }
        window.head_hour = now_hour;
    }

    fn window_sum(window: &UsageWindow) -> Result<i128, Error> {
        window.amounts.iter().try_fold(0i128, |sum, amount| {
            sum.checked_add(amount).ok_or(Error::DailyCapExceeded)
        })
    }

    /// Advances `window` to `now`, then adds `amount` to the current hour if
    /// the window total stays within `max_daily`. Returns the new total.
    fn consume_window(
        window: &mut UsageWindow,
        now: u64,
        amount: i128,
        max_daily: i128,
    ) -> Result<i128, Error> {
        Self::advance(window, now / BUCKET_SECONDS);
        let total = Self::window_sum(window)?
            .checked_add(amount)
            .ok_or(Error::DailyCapExceeded)?;
        if total > max_daily {
            return Err(Error::DailyCapExceeded);
        }
        let head = window
            .amounts
            .get(window.head_idx)
            .ok_or(Error::NotInitialized)?;
        window.amounts.set(
            window.head_idx,
            head.checked_add(amount).ok_or(Error::DailyCapExceeded)?,
        );
        Ok(total)
    }

    /// Checks the per-market and daily caps for `amount` and, if both pass,
    /// records it. Called *after* the market-core call it authorizes:
    /// Soroban invocations are atomic, so if this returns `Err` the whole
    /// transaction (including the trade already made) reverts.
    ///
    /// The daily cap is a sliding window: usage in the trade's hour and the
    /// 24 hours before it (see `BUCKETS`) may never exceed `max_daily`, so
    /// no trailing 24h interval can either. `TradeAuthorized.daily_total` is
    /// that window sum after the trade.
    fn reserve_capacity(
        env: &Env,
        config: &RiskConfig,
        market_id: u64,
        amount: i128,
    ) -> Result<(), Error> {
        let mut window = Self::load_window(env)?;
        let mut exposure = Self::market_exposure(env, market_id);
        let new_market = exposure
            .amount
            .checked_add(amount)
            .ok_or(Error::MarketCapExceeded)?;
        if new_market > config.max_market {
            return Err(Error::MarketCapExceeded);
        }
        let new_daily = Self::consume_window(
            &mut window,
            env.ledger().timestamp(),
            amount,
            config.max_daily,
        )?;
        exposure.amount = new_market;
        exposure.open = true;
        Self::save_market(env, market_id, &exposure);
        let storage = env.storage().instance();
        storage.set(&DataKey::UsageWindow, &window);
        storage.remove(&DataKey::Daily);
        // The window must outlive idle days, so keep the instance live.
        storage.extend_ttl(BUMP_THRESHOLD, BUMP_TO);
        TradeAuthorized {
            market_id,
            agent: config.agent.clone(),
            amount,
            market_total: new_market,
            daily_total: new_daily,
        }
        .publish(env);
        Ok(())
    }

    // ----- net exposure ------------------------------------------------------
    // Two caps with different questions: `max_market` limits what was *ever*
    // committed to one market (lifetime, never freed); `max_net_exposure`
    // limits what is *currently* committed across all markets (cost basis,
    // freed by sells, redemptions and claims). Enforcement reads only local
    // counters, never a pool price: a spot-valued cap would let anyone pump
    // the price to unlock headroom, or crash it to block the agent.

    fn load_net(env: &Env, market_id: u64) -> NetPosition {
        env.storage()
            .persistent()
            .get(&DataKey::Net(market_id))
            .unwrap_or(NetPosition {
                collateral_in: 0,
                collateral_out: 0,
            })
    }

    /// Stores `next` for `market_id`, moves `NetTotal` by the change in that
    /// market's cost basis (so it never needs iterating) and returns the new
    /// total.
    fn save_net(
        env: &Env,
        market_id: u64,
        before: &NetPosition,
        next: &NetPosition,
    ) -> Result<i128, Error> {
        let total = Self::net_exposure(env.clone())
            .checked_sub(before.cost_basis())
            .and_then(|t| t.checked_add(next.cost_basis()))
            .ok_or(Error::NetExposureExceeded)?;
        let key = DataKey::Net(market_id);
        env.storage().persistent().set(&key, next);
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
        env.storage().instance().set(&DataKey::NetTotal, &total);
        NetExposureUpdated {
            market_id,
            net_market: next.cost_basis(),
            net_total: total,
        }
        .publish(env);
        Ok(total)
    }

    /// Records `amount` sent into `market_id` and enforces `max_net_exposure`
    /// on the new total. Like `reserve_capacity`, runs after the market-core
    /// call and relies on the invocation reverting on `Err`.
    fn reserve_net(env: &Env, market_id: u64, amount: i128) -> Result<(), Error> {
        let before = Self::load_net(env, market_id);
        let next = NetPosition {
            collateral_in: before
                .collateral_in
                .checked_add(amount)
                .ok_or(Error::NetExposureExceeded)?,
            ..before
        };
        let total = Self::save_net(env, market_id, &before, &next)?;
        if total > Self::get_net_cap(env.clone())? {
            return Err(Error::NetExposureExceeded);
        }
        Ok(())
    }

    /// Records `amount` received back from `market_id` (a sell, redemption,
    /// pool remainder or LP fee claim).
    fn record_return(env: &Env, market_id: u64, amount: i128) -> Result<(), Error> {
        if amount <= 0 {
            return Ok(());
        }
        let before = Self::load_net(env, market_id);
        let next = NetPosition {
            collateral_out: before
                .collateral_out
                .checked_add(amount)
                .ok_or(Error::InvalidAmount)?,
            ..before
        };
        Self::save_net(env, market_id, &before, &next)?;
        Ok(())
    }

    /// Drops `market_id` from net accounting (its holdings left the treasury).
    fn clear_net(env: &Env, market_id: u64) -> Result<(), Error> {
        let before = Self::load_net(env, market_id);
        let cleared = NetPosition {
            collateral_in: 0,
            collateral_out: 0,
        };
        Self::save_net(env, market_id, &before, &cleared)?;
        Ok(())
    }

    /// Private mirror of market-core's `math::settle_value` for final markets.
    fn settle_value(status: &MarketStatus, yes: i128, no: i128) -> i128 {
        match status {
            MarketStatus::Resolved(Outcome::Yes) => yes,
            MarketStatus::Resolved(Outcome::No) => no,
            MarketStatus::Void => yes.saturating_add(no) / 2,
            MarketStatus::Open => 0,
        }
    }

    /// Pre-authorizes the SEP-41 `transfer(from, to, amount)` that
    /// market-core will make from the treasury's own balance when the
    /// treasury calls one of its trading entrypoints. See the comment on
    /// `agent_buy` for why this is required.
    fn authorize_token_pull(env: &Env, config: &RiskConfig, from: &Address, amount: i128) {
        env.authorize_as_current_contract(vec![
            env,
            InvokerContractAuthEntry::Contract(SubContractInvocation {
                context: ContractContext {
                    contract: config.collateral.clone(),
                    fn_name: Symbol::new(env, "transfer"),
                    args: vec![
                        env,
                        from.into_val(env),
                        config.market_core.into_val(env),
                        amount.into_val(env),
                    ],
                },
                sub_invocations: vec![env],
            }),
        ]);
    }

    fn market_exposure(env: &Env, market_id: u64) -> MarketExposure {
        env.storage()
            .persistent()
            .get(&DataKey::Market(market_id))
            .unwrap_or(MarketExposure {
                amount: 0,
                open: false,
            })
    }

    fn save_market(env: &Env, market_id: u64, exposure: &MarketExposure) {
        let key = DataKey::Market(market_id);
        env.storage().persistent().set(&key, exposure);
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
    }
}

#[cfg(test)]
mod test {
    extern crate std;

    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger, MockAuth, MockAuthInvoke},
        token::StellarAssetClient,
        BytesN, String,
    };

    // Comfortably past every timestamp advance the tests make, so a market
    // never closes out from under a trade mid-test.
    const MARKET_CLOSE: u64 = 1_000 + 30 * DAY_SECONDS;

    struct Fixture {
        env: Env,
        admin: Address,
        treasury: TreasuryClient<'static>,
        treasury_id: Address,
        token: Address,
        market_core: MarketCoreClient<'static>,
        market_id: u64,
    }

    fn fixture() -> Fixture {
        fixture_with_caps(
            20 * 10_000_000_i128,
            50 * 10_000_000_i128,
            60 * 10_000_000_i128,
        )
    }

    fn fixture_with_caps(max_trade: i128, max_market: i128, max_daily: i128) -> Fixture {
        fixture_with_net_cap(max_trade, max_market, max_daily, max_daily)
    }

    fn fixture_with_net_cap(
        max_trade: i128,
        max_market: i128,
        max_daily: i128,
        max_net_exposure: i128,
    ) -> Fixture {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_000);
        let admin = Address::generate(&env);
        let agent = Address::generate(&env);
        let creator = Address::generate(&env);
        let treasury_id = Address::generate(&env);
        let collateral = env.register_stellar_asset_contract_v2(admin.clone());
        let token = StellarAssetClient::new(&env, &collateral.address());
        token.mint(&creator, &(1_000 * 10_000_000_i128));
        // Treasury needs real capital: agent_buy/agent_create_market move
        // actual SEP-41 balance, they are not just an advisory cap check.
        token.mint(&treasury_id, &(1_000 * 10_000_000_i128));

        let market_core_id = env.register(
            market_core::WASM,
            (
                admin.clone(),
                collateral.address(),
                treasury_id.clone(),
                Address::generate(&env),
                100u32,
                // Protocol keeps the whole fee; treasury tests only exercise
                // protocol-side sweeps.
                10_000u32,
            ),
        );
        let market_core = MarketCoreClient::new(&env, &market_core_id);
        let market_id = market_core.create_market(
            &creator,
            &BytesN::from_array(&env, &[11u8; 32]),
            &BytesN::from_array(&env, &[111u8; 32]),
            &String::from_str(&env, "ipfs://treasury-market"),
            &Category::Crypto,
            &MARKET_CLOSE,
            &(100 * 10_000_000_i128),
            &5000u32,
        );
        let treasury = env.register_at(
            &treasury_id,
            Treasury,
            TreasuryArgs::__constructor(
                &admin,
                &agent,
                &collateral.address(),
                &market_core_id,
                &max_trade,
                &max_market,
                &max_daily,
                &max_net_exposure,
            ),
        );
        Fixture {
            env: env.clone(),
            admin,
            treasury: TreasuryClient::new(&env, &treasury),
            treasury_id,
            token: collateral.address(),
            market_core,
            market_id,
        }
    }

    #[test]
    fn enforces_caps_and_closes_exposure() {
        let f = fixture();
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        assert!(f
            .treasury
            .try_agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0)
            .is_ok());
        assert_eq!(
            f.treasury
                .try_agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0),
            Err(Ok(Error::MarketCapExceeded))
        );
        assert_eq!(
            f.treasury.try_close_trade(&f.market_id),
            Ok(Ok(40 * 10_000_000_i128))
        );
        assert!(f.treasury.daily_usage().amount == 40 * 10_000_000_i128);
    }

    #[test]
    fn rejects_trade_above_single_trade_cap() {
        let f = fixture();
        assert_eq!(
            f.treasury
                .try_agent_buy(&f.market_id, &Outcome::Yes, &(21 * 10_000_000_i128), &0),
            Err(Ok(Error::TradeCapExceeded))
        );
    }

    #[test]
    fn rejects_trade_above_daily_cap() {
        let f = fixture();
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        f.treasury.close_trade(&f.market_id);
        let second_market = f.market_core.create_market(
            &f.market_core.get_market(&f.market_id).creator,
            &BytesN::from_array(&f.env, &[12u8; 32]),
            &BytesN::from_array(&f.env, &[112u8; 32]),
            &String::from_str(&f.env, "ipfs://treasury-market-2"),
            &Category::Crypto,
            &MARKET_CLOSE,
            &(100 * 10_000_000_i128),
            &5000u32,
        );
        f.treasury
            .agent_buy(&second_market, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        f.treasury.close_trade(&second_market);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        f.treasury.close_trade(&f.market_id);
        // 60 already used today (the daily cap); one more real trade of any
        // size must be rejected for exceeding it.
        assert_eq!(
            f.treasury
                .try_agent_buy(&f.market_id, &Outcome::Yes, &(10 * 10_000_000_i128), &0),
            Err(Ok(Error::DailyCapExceeded))
        );
    }

    /// Replaces the old `daily_cap_resets_after_one_day`, which pinned the
    /// fixed window's reset. With 25 hourly buckets a trade from hour 0 still
    /// counts exactly 24h later (hour 24's window is hours 0..=24) and only
    /// drops out once the head reaches hour 25.
    #[test]
    fn daily_cap_frees_capacity_after_window_slides() {
        let f = fixture();
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        f.treasury.close_trade(&f.market_id);
        f.env.ledger().set_timestamp(1_000 + DAY_SECONDS);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        assert_eq!(f.treasury.daily_usage().amount, 40 * 10_000_000_i128);
        f.env.ledger().set_timestamp(25 * BUCKET_SECONDS);
        assert_eq!(f.treasury.daily_usage().amount, 20 * 10_000_000_i128);
    }

    #[test]
    fn collects_market_fees_into_treasury() {
        let f = fixture();
        let trader = Address::generate(&f.env);
        StellarAssetClient::new(&f.env, &f.token).mint(&trader, &(100 * 10_000_000_i128));
        let market = f.market_core.get_market(&f.market_id);
        let trader_token = soroban_sdk::token::TokenClient::new(&f.env, &f.token);
        let before = trader_token.balance(&trader);
        f.market_core.buy(
            &trader,
            &f.market_id,
            &Outcome::Yes,
            &(10 * 10_000_000_i128),
            &0,
        );
        let fee = market.fee_bps as i128;
        assert!(fee > 0);
        let treasury_before =
            soroban_sdk::token::TokenClient::new(&f.env, &f.token).balance(&f.treasury_id);
        let collected = f.treasury.collect_market_fees(&f.market_id);
        assert!(collected > 0);
        assert_eq!(
            soroban_sdk::token::TokenClient::new(&f.env, &f.token).balance(&f.treasury_id),
            treasury_before + collected
        );
        assert!(trader_token.balance(&trader) < before);
    }

    #[test]
    fn agent_create_market_seeds_from_treasury_capital() {
        // market-core enforces a 100 USDC MIN_SEED, so this needs a treasury
        // whose max_trade allows seeding (the shared `fixture()` deliberately
        // uses a tighter 20 USDC cap for the trade-limit tests).
        let f = fixture_with_caps(
            150 * 10_000_000_i128,
            300 * 10_000_000_i128,
            300 * 10_000_000_i128,
        );
        let token = soroban_sdk::token::TokenClient::new(&f.env, &f.token);
        let before = token.balance(&f.treasury_id);
        let seed = 100 * 10_000_000_i128;

        let market_id = f.treasury.agent_create_market(
            &BytesN::from_array(&f.env, &[13u8; 32]),
            &BytesN::from_array(&f.env, &[113u8; 32]),
            &String::from_str(&f.env, "ipfs://agent-created"),
            &Category::Macro,
            &MARKET_CLOSE,
            &seed,
            &5000u32,
        );

        let market = f.market_core.get_market(&market_id);
        assert_eq!(market.creator, f.treasury_id);
        assert_eq!(token.balance(&f.treasury_id), before - seed);
        assert_eq!(f.treasury.daily_usage().amount, seed);
    }

    #[test]
    fn agent_create_market_rejects_seed_above_trade_cap() {
        let f = fixture_with_caps(
            150 * 10_000_000_i128,
            300 * 10_000_000_i128,
            300 * 10_000_000_i128,
        );
        assert_eq!(
            f.treasury.try_agent_create_market(
                &BytesN::from_array(&f.env, &[14u8; 32]),
                &BytesN::from_array(&f.env, &[114u8; 32]),
                &String::from_str(&f.env, "ipfs://agent-created-2"),
                &Category::Macro,
                &MARKET_CLOSE,
                &(151 * 10_000_000_i128),
                &5000u32,
            ),
            Err(Ok(Error::TradeCapExceeded))
        );
    }

    #[test]
    fn agent_sell_returns_capital_without_touching_caps() {
        let f = fixture();
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        let quote = f
            .market_core
            .quote_sell(&f.market_id, &Outcome::Yes, &(5 * 10_000_000_i128));
        let shares_in =
            f.treasury
                .agent_sell(&f.market_id, &Outcome::Yes, &(5 * 10_000_000_i128), &quote);
        assert!(shares_in > 0);
        // Selling must not consume any more of the daily/market caps.
        assert_eq!(f.treasury.daily_usage().amount, 20 * 10_000_000_i128);
    }

    #[test]
    fn paused_treasury_rejects_agent_trading() {
        let f = fixture();
        f.treasury.set_paused(&true);
        assert_eq!(
            f.treasury
                .try_agent_buy(&f.market_id, &Outcome::Yes, &(10_000_000_i128), &0),
            Err(Ok(Error::Paused))
        );
    }

    #[test]
    fn wrong_caller_cannot_pause_or_withdraw() {
        let f = fixture();
        let not_admin = Address::generate(&f.env);
        f.env.set_auths(&[]);
        assert!(f
            .treasury
            .mock_auths(&[MockAuth {
                address: &not_admin,
                invoke: &MockAuthInvoke {
                    contract: &f.treasury_id,
                    fn_name: "set_paused",
                    args: (true,).into_val(&f.env),
                    sub_invokes: &[],
                },
            }])
            .try_set_paused(&true)
            .is_err());
        assert!(f
            .treasury
            .mock_auths(&[MockAuth {
                address: &not_admin,
                invoke: &MockAuthInvoke {
                    contract: &f.treasury_id,
                    fn_name: "withdraw",
                    args: (not_admin.clone(), 1_i128).into_val(&f.env),
                    sub_invokes: &[],
                },
            }])
            .try_withdraw(&not_admin, &1_i128)
            .is_err());
    }

    /// The pitfall the design doc calls out explicitly: when the treasury
    /// calls `market_core.buy`, market-core in turn calls
    /// `token.transfer(treasury -> market_core)`. That nested transfer needs
    /// the treasury to pre-authorize it via `authorize_as_current_contract`
    /// (see `Treasury::authorize_token_pull`), or the whole call fails
    /// authorization.
    ///
    /// This test deliberately uses `mock_auths` for only the agent's
    /// top-level call — not `mock_all_auths`, which would rubber-stamp the
    /// nested transfer too and hide a regression. Verified by hand: commenting
    /// out the `authorize_token_pull` call in `agent_buy` makes this test
    /// fail with an authorization error; restoring it makes it pass again.
    #[test]
    fn pitfall_agent_buy_requires_treasury_self_authorization_for_nested_transfer() {
        let f = fixture();
        let agent = f.treasury.get_config().agent;
        let market_id = f.market_id;
        let outcome = Outcome::Yes;
        let amount = 20 * 10_000_000_i128;
        let min_out = 0_i128;

        f.env.set_auths(&[]);
        let shares = f
            .treasury
            .mock_auths(&[MockAuth {
                address: &agent,
                invoke: &MockAuthInvoke {
                    contract: &f.treasury_id,
                    fn_name: "agent_buy",
                    args: (market_id, outcome.clone(), amount, min_out).into_val(&f.env),
                    sub_invokes: &[],
                },
            }])
            .agent_buy(&market_id, &outcome, &amount, &min_out);
        assert!(shares > 0);
    }

    #[test]
    fn agent_claims_lp_fees_for_own_market() {
        let f = fixture_with_caps(
            150 * 10_000_000_i128,
            300 * 10_000_000_i128,
            300 * 10_000_000_i128,
        );
        // New markets snapshot the split at creation; give the creator half.
        f.market_core.set_protocol_fee_share_bps(&5000u32);
        let market_id = f.treasury.agent_create_market(
            &BytesN::from_array(&f.env, &[15u8; 32]),
            &BytesN::from_array(&f.env, &[115u8; 32]),
            &String::from_str(&f.env, "ipfs://lp-fees"),
            &Category::Macro,
            &MARKET_CLOSE,
            &(100 * 10_000_000_i128),
            &5000u32,
        );
        let trader = Address::generate(&f.env);
        StellarAssetClient::new(&f.env, &f.token).mint(&trader, &(100 * 10_000_000_i128));
        f.market_core.buy(
            &trader,
            &market_id,
            &Outcome::Yes,
            &(10 * 10_000_000_i128),
            &0_i128,
        );

        // Fee is 1_000_000, split 50/50 between protocol and the LP.
        let token = soroban_sdk::token::TokenClient::new(&f.env, &f.token);
        let before = token.balance(&f.treasury_id);
        assert_eq!(f.treasury.agent_claim_lp_fees(&market_id), 500_000);
        assert_eq!(token.balance(&f.treasury_id), before + 500_000);
        assert_eq!(f.treasury.agent_claim_lp_fees(&market_id), 0);
    }

    #[test]
    fn agent_buy_breaker_revert_leaves_caps_unchanged() {
        let f = fixture();
        f.market_core
            .set_default_breaker(&market_core::BreakerConfig {
                max_impact_bps: 10,
                band_bps: 0,
            });
        let creator = f.market_core.get_market(&f.market_id).creator;
        let market_id = f.market_core.create_market(
            &creator,
            &BytesN::from_array(&f.env, &[16u8; 32]),
            &BytesN::from_array(&f.env, &[116u8; 32]),
            &String::from_str(&f.env, "ipfs://breaker-market"),
            &Category::Crypto,
            &MARKET_CLOSE,
            &(100 * 10_000_000_i128),
            &5000u32,
        );

        // 0.1 USDC moves the price ~4 bps: inside the 10 bps impact cap.
        f.treasury
            .agent_buy(&market_id, &Outcome::Yes, &(1_000_000_i128), &0);
        assert_eq!(f.treasury.daily_usage().amount, 1_000_000);

        // 20 USDC moves it ~893 bps: market-core trips the breaker, which
        // reverts the whole treasury call — the caps must not absorb it.
        assert!(f
            .treasury
            .try_agent_buy(&market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0)
            .is_err());
        assert_eq!(f.treasury.daily_usage().amount, 1_000_000);
        let exposure = f.env.as_contract(&f.treasury_id, || {
            f.env
                .storage()
                .persistent()
                .get::<DataKey, MarketExposure>(&DataKey::Market(market_id))
                .unwrap()
        });
        assert!(exposure.open);
        assert_eq!(exposure.amount, 1_000_000);
    }

    #[test]
    fn agent_buy_v2_forwards_bounds() {
        let f = fixture();
        let now = f.env.ledger().timestamp();
        // Expired deadline.
        assert!(f
            .treasury
            .try_agent_buy_v2(
                &f.market_id,
                &Outcome::Yes,
                &(10_000_000_i128),
                &0,
                &0u32,
                &(now - 1),
            )
            .is_err());
        // Average fill for 20 USDC is ~5505 bps; limit 5000 must reject.
        assert!(f
            .treasury
            .try_agent_buy_v2(
                &f.market_id,
                &Outcome::Yes,
                &(20 * 10_000_000_i128),
                &0,
                &5000u32,
                &(now + 60),
            )
            .is_err());
        assert_eq!(f.treasury.daily_usage().amount, 0);

        // Generous bounds: succeeds and consumes the caps like agent_buy.
        let shares = f.treasury.agent_buy_v2(
            &f.market_id,
            &Outcome::Yes,
            &(20 * 10_000_000_i128),
            &0,
            &6000u32,
            &(now + 60),
        );
        assert!(shares > 0);
        assert_eq!(f.treasury.daily_usage().amount, 20 * 10_000_000_i128);

        // The sell side forwards its bounds too.
        let quote = f
            .market_core
            .quote_sell(&f.market_id, &Outcome::Yes, &(5 * 10_000_000_i128));
        let sold = f.treasury.agent_sell_v2(
            &f.market_id,
            &Outcome::Yes,
            &(5 * 10_000_000_i128),
            &quote,
            &0u32,
            &(now + 60),
        );
        assert!(sold > 0);
        assert!(f
            .treasury
            .try_agent_sell_v2(
                &f.market_id,
                &Outcome::Yes,
                &(5 * 10_000_000_i128),
                &quote,
                &0u32,
                &(now - 1),
            )
            .is_err());
    }

    #[test]
    fn agent_buy_v2_deadline_revert_leaves_caps_unchanged() {
        let f = fixture();
        let past = f.env.ledger().timestamp() - 1;
        assert!(f
            .treasury
            .try_agent_buy_v2(
                &f.market_id,
                &Outcome::Yes,
                &(10 * 10_000_000_i128),
                &0,
                &0u32,
                &past,
            )
            .is_err());
        assert_eq!(f.treasury.daily_usage().amount, 0);
        let exposure = f.env.as_contract(&f.treasury_id, || {
            f.env
                .storage()
                .persistent()
                .get::<DataKey, MarketExposure>(&DataKey::Market(f.market_id))
        });
        assert_eq!(exposure, None);
    }

    /// The v2 twin of the pitfall test above: the `buy_v2` path must also
    /// pre-authorize the nested token transfer (`mock_auths` for only the
    /// agent, so a regression in `authorize_token_pull` fails here).
    #[test]
    fn pitfall_agent_buy_v2_forwards_nested_transfer_auth() {
        let f = fixture();
        let agent = f.treasury.get_config().agent;
        let market_id = f.market_id;
        let outcome = Outcome::Yes;
        let amount = 20 * 10_000_000_i128;
        let min_out = 0_i128;
        let limit = 0u32;
        let deadline = f.env.ledger().timestamp() + 60;

        f.env.set_auths(&[]);
        let shares = f
            .treasury
            .mock_auths(&[MockAuth {
                address: &agent,
                invoke: &MockAuthInvoke {
                    contract: &f.treasury_id,
                    fn_name: "agent_buy_v2",
                    args: (market_id, outcome.clone(), amount, min_out, limit, deadline)
                        .into_val(&f.env),
                    sub_invokes: &[],
                },
            }])
            .agent_buy_v2(&market_id, &outcome, &amount, &min_out, &limit, &deadline);
        assert!(shares > 0);
    }

    #[test]
    fn migrate_market_moves_position_and_lp_to_successor() {
        let f = fixture_with_caps(
            150 * 10_000_000_i128,
            300 * 10_000_000_i128,
            300 * 10_000_000_i128,
        );
        let market_id = f.treasury.agent_create_market(
            &BytesN::from_array(&f.env, &[17u8; 32]),
            &BytesN::from_array(&f.env, &[117u8; 32]),
            &String::from_str(&f.env, "ipfs://migration"),
            &Category::Macro,
            &MARKET_CLOSE,
            &(100 * 10_000_000_i128),
            &5000u32,
        );
        f.treasury
            .agent_buy(&market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        let held = f.market_core.get_position(&market_id, &f.treasury_id).yes;
        assert!(held > 0);
        assert_eq!(f.market_core.get_market(&market_id).creator, f.treasury_id);

        let successor = Address::generate(&f.env);
        f.treasury.set_successor(&successor);
        f.treasury.migrate_market(&market_id);

        assert_eq!(
            f.market_core.get_position(&market_id, &f.treasury_id).yes,
            0
        );
        assert_eq!(f.market_core.get_position(&market_id, &successor).yes, held);
        assert_eq!(f.market_core.get_market(&market_id).creator, successor);
        let exposure = f.env.as_contract(&f.treasury_id, || {
            f.env
                .storage()
                .persistent()
                .get::<DataKey, MarketExposure>(&DataKey::Market(market_id))
                .unwrap()
        });
        assert!(!exposure.open);
        // Migration does not free the cumulative cap (like close_trade).
        assert_eq!(exposure.amount, 120 * 10_000_000_i128);
    }

    #[test]
    fn migrate_requires_successor() {
        let f = fixture();
        assert_eq!(
            f.treasury.try_migrate_market(&f.market_id),
            Err(Ok(Error::NoSuccessor))
        );
    }

    #[test]
    fn agent_cannot_migrate() {
        let f = fixture();
        let agent = f.treasury.get_config().agent;
        f.env.set_auths(&[]);
        assert!(f
            .treasury
            .mock_auths(&[MockAuth {
                address: &agent,
                invoke: &MockAuthInvoke {
                    contract: &f.treasury_id,
                    fn_name: "migrate_market",
                    args: (f.market_id,).into_val(&f.env),
                    sub_invokes: &[],
                },
            }])
            .try_migrate_market(&f.market_id)
            .is_err());
    }

    /// The treasury's calls to market-core's `transfer_position` /
    /// `transfer_lp` are *direct* calls, so market-core's `require_auth`
    /// on the treasury is satisfied implicitly by the invoker rule — no
    /// `authorize_as_current_contract` entry is needed (unlike the nested
    /// token transfer in `agent_buy`). Mocking only the admin's auth must be
    /// enough for the whole migration.
    #[test]
    fn pitfall_migrate_market_needs_only_admin_auth() {
        let f = fixture_with_caps(
            150 * 10_000_000_i128,
            300 * 10_000_000_i128,
            300 * 10_000_000_i128,
        );
        let market_id = f.treasury.agent_create_market(
            &BytesN::from_array(&f.env, &[18u8; 32]),
            &BytesN::from_array(&f.env, &[118u8; 32]),
            &String::from_str(&f.env, "ipfs://migration-auth"),
            &Category::Macro,
            &MARKET_CLOSE,
            &(100 * 10_000_000_i128),
            &5000u32,
        );
        f.treasury
            .agent_buy(&market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        let successor = Address::generate(&f.env);
        f.treasury.set_successor(&successor);
        let held = f.market_core.get_position(&market_id, &f.treasury_id).yes;

        f.env.set_auths(&[]);
        f.treasury
            .mock_auths(&[MockAuth {
                address: &f.admin,
                invoke: &MockAuthInvoke {
                    contract: &f.treasury_id,
                    fn_name: "migrate_market",
                    args: (market_id,).into_val(&f.env),
                    sub_invokes: &[],
                },
            }])
            .migrate_market(&market_id);

        assert_eq!(f.market_core.get_position(&market_id, &successor).yes, held);
        assert_eq!(f.market_core.get_market(&market_id).creator, successor);
    }

    #[test]
    fn migrate_then_successor_redeems_after_resolution() {
        let f = fixture();
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * 10_000_000_i128), &0);
        let successor = Address::generate(&f.env);
        f.treasury.set_successor(&successor);

        f.env.ledger().set_timestamp(MARKET_CLOSE + 1);
        f.market_core.resolve(&f.market_id, &Outcome::Yes);
        f.treasury.migrate_market(&f.market_id);

        // The successor (not the treasury) now redeems the migrated shares.
        let payout = f.market_core.redeem(&successor, &f.market_id);
        assert!(payout > 0);
        assert_eq!(
            soroban_sdk::token::TokenClient::new(&f.env, &f.token).balance(&successor),
            payout
        );
        assert_eq!(
            f.market_core.get_position(&f.market_id, &f.treasury_id).yes,
            0
        );
    }

    // ----- sliding daily window -------------------------------------------

    const USDC: i128 = 10_000_000;

    /// A second externally-created market in the fixture's market-core.
    fn extra_market(f: &Fixture, tag: u8) -> u64 {
        f.market_core.create_market(
            &f.market_core.get_market(&f.market_id).creator,
            &BytesN::from_array(&f.env, &[tag; 32]),
            &BytesN::from_array(&f.env, &[tag.wrapping_add(100); 32]),
            &String::from_str(&f.env, "ipfs://treasury-extra"),
            &Category::Crypto,
            &MARKET_CLOSE,
            &(100 * USDC),
            &5000u32,
        )
    }

    fn stored_window(f: &Fixture) -> Option<UsageWindow> {
        f.env.as_contract(&f.treasury_id, || {
            f.env.storage().instance().get(&DataKey::UsageWindow)
        })
    }

    #[test]
    fn burst_across_old_window_boundary_is_rejected() {
        let f = fixture_with_caps(20 * USDC, 60 * USDC, 60 * USDC);
        // The old fixed window began at 1_000 and reset at 1_000 + 86_400.
        f.env.ledger().set_timestamp(1_000 + 86_399);
        for _ in 0..3 {
            f.treasury
                .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        }
        f.env.ledger().set_timestamp(1_000 + 86_401);
        let other = extra_market(&f, 21);
        assert_eq!(
            f.treasury.try_agent_buy(&other, &Outcome::Yes, &USDC, &0),
            Err(Ok(Error::DailyCapExceeded))
        );
    }

    #[test]
    fn usage_expires_after_25_hours() {
        let f = fixture();
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        // Hour 0 is still in hour 24's window...
        f.env.ledger().set_timestamp(24 * BUCKET_SECONDS + 3_599);
        assert_eq!(f.treasury.daily_headroom(), 40 * USDC);
        // ...and gone from hour 25's.
        f.env.ledger().set_timestamp(25 * BUCKET_SECONDS);
        assert_eq!(f.treasury.daily_headroom(), 60 * USDC);
    }

    #[test]
    fn same_hour_trades_accumulate() {
        let f = fixture();
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        f.env.ledger().set_timestamp(3_599);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::No, &(15 * USDC), &0);
        let window = f.treasury.daily_usage_detail();
        assert_eq!(window.head_hour, 0);
        assert_eq!(window.amounts.get(window.head_idx), Some(35 * USDC));
        assert_eq!(window.amounts.iter().sum::<i128>(), 35 * USDC);
    }

    #[test]
    fn long_idle_gap_clears_all_buckets() {
        let f = fixture();
        for hour in [0u64, 5, 20] {
            f.env.ledger().set_timestamp(hour * BUCKET_SECONDS + 1_000);
            f.treasury
                .agent_buy(&f.market_id, &Outcome::Yes, &(10 * USDC), &0);
        }
        f.env
            .ledger()
            .set_timestamp(20 * BUCKET_SECONDS + 10 * DAY_SECONDS);
        let window = f.treasury.daily_usage_detail();
        assert_eq!(window.amounts.len(), BUCKETS);
        assert!(window.amounts.iter().all(|a| a == 0));
        assert_eq!(f.treasury.daily_usage().amount, 0);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        assert_eq!(f.treasury.daily_usage().amount, 20 * USDC);
    }

    #[test]
    fn daily_usage_view_is_pure_and_stable() {
        let f = fixture();
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        let stored = stored_window(&f);
        f.env.ledger().set_timestamp(5 * BUCKET_SECONDS);
        let first = f.treasury.daily_usage();
        let second = f.treasury.daily_usage();
        assert_eq!(first, second);
        assert_eq!(
            first,
            DailyUsage {
                day_start: 0,
                amount: 20 * USDC
            }
        );
        // Reading advanced a copy only.
        assert_eq!(stored_window(&f), stored);
        assert_eq!(stored.unwrap().head_hour, 0);
    }

    #[test]
    fn legacy_daily_entry_is_migrated_conservatively() {
        let f = fixture();
        let install_legacy = |day_start: u64, amount: i128| {
            f.env.as_contract(&f.treasury_id, || {
                let storage = f.env.storage().instance();
                storage.remove(&DataKey::UsageWindow);
                storage.set(&DataKey::Daily, &DailyUsage { day_start, amount });
            })
        };

        // Legacy day still running: its usage counts in full.
        install_legacy(1_000, 50 * USDC);
        f.env.ledger().set_timestamp(1_000 + 23 * BUCKET_SECONDS);
        assert_eq!(f.treasury.daily_usage().amount, 50 * USDC);
        let other = extra_market(&f, 22);
        assert_eq!(
            f.treasury
                .try_agent_buy(&other, &Outcome::Yes, &(20 * USDC), &0),
            Err(Ok(Error::DailyCapExceeded))
        );
        f.treasury
            .agent_buy(&other, &Outcome::Yes, &(10 * USDC), &0);
        f.env.as_contract(&f.treasury_id, || {
            assert!(!f.env.storage().instance().has(&DataKey::Daily));
        });
        let window = stored_window(&f).unwrap();
        assert_eq!(window.amounts.iter().sum::<i128>(), 60 * USDC);

        // Legacy day already over: the window starts empty.
        install_legacy(1_000, 50 * USDC);
        f.env.ledger().set_timestamp(1_000 + DAY_SECONDS);
        assert_eq!(f.treasury.daily_usage().amount, 0);
    }

    #[test]
    fn daily_headroom_matches_cap_minus_window_sum() {
        let f = fixture();
        assert_eq!(f.treasury.daily_headroom(), 60 * USDC);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        f.env.ledger().set_timestamp(2 * BUCKET_SECONDS);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::No, &(5 * USDC), &0);
        assert_eq!(
            f.treasury.daily_headroom(),
            60 * USDC - f.treasury.daily_usage().amount
        );
        assert_eq!(f.treasury.daily_headroom(), 35 * USDC);
    }

    /// Randomized schedule (an LCG, as in market-core's math property tests)
    /// of 2_000 attempted trades over 30 days against the window logic
    /// itself. After every accepted trade the true usage over the trailing
    /// `(t - 86_400, t]` interval, brute-forced from a log, stays within cap.
    #[test]
    fn sliding_window_property_random_schedule() {
        let env = Env::default();
        env.cost_estimate().budget().reset_unlimited();
        let max_daily: i128 = 1_000;
        let mut window = Treasury::empty_window(&env, 0);
        let mut log: std::vec::Vec<(u64, i128)> = std::vec::Vec::new();
        let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            seed >> 33
        };
        let mut now: u64 = 0;
        let mut accepted = 0;
        for _ in 0..2_000 {
            // Mostly short gaps (bursts), sometimes multi-hour ones.
            now += if next() % 4 == 0 {
                next() % (6 * BUCKET_SECONDS)
            } else {
                next() % 900
            };
            if now > 30 * DAY_SECONDS {
                break;
            }
            let amount = 1 + (next() % 300) as i128;
            if Treasury::consume_window(&mut window, now, amount, max_daily).is_ok() {
                accepted += 1;
                log.push((now, amount));
                let trailing: i128 = log
                    .iter()
                    .filter(|(t, _)| *t + DAY_SECONDS > now)
                    .map(|(_, a)| a)
                    .sum();
                assert!(trailing <= max_daily, "t={now}: {trailing} > {max_daily}");
            }
        }
        assert!(accepted > 100, "schedule too sparse: {accepted}");
    }

    // ----- net exposure -----------------------------------------------------

    fn net_fixture(max_net_exposure: i128) -> Fixture {
        fixture_with_net_cap(150 * USDC, 300 * USDC, 300 * USDC, max_net_exposure)
    }

    fn agent_market(f: &Fixture, tag: u8) -> u64 {
        f.treasury.agent_create_market(
            &BytesN::from_array(&f.env, &[tag; 32]),
            &BytesN::from_array(&f.env, &[tag.wrapping_add(100); 32]),
            &String::from_str(&f.env, "ipfs://net"),
            &Category::Macro,
            &MARKET_CLOSE,
            &(100 * USDC),
            &5000u32,
        )
    }

    #[test]
    fn net_cap_blocks_buy_when_total_deployed_exceeds_limit() {
        let f = fixture_with_net_cap(20 * USDC, 50 * USDC, 60 * USDC, 30 * USDC);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        let other = extra_market(&f, 23);
        // Within every lifetime/daily cap, but 20 + 11 > 30 deployed.
        assert_eq!(
            f.treasury
                .try_agent_buy(&other, &Outcome::Yes, &(11 * USDC), &0),
            Err(Ok(Error::NetExposureExceeded))
        );
        f.treasury
            .agent_buy(&other, &Outcome::Yes, &(10 * USDC), &0);
        assert_eq!(f.treasury.net_exposure(), 30 * USDC);
        assert_eq!(f.treasury.get_net_cap(), 30 * USDC);
    }

    #[test]
    fn net_cap_applies_to_market_creation() {
        let f = fixture_with_net_cap(100 * USDC, 300 * USDC, 300 * USDC, 150 * USDC);
        agent_market(&f, 24);
        assert_eq!(f.treasury.net_exposure(), 100 * USDC);
        assert_eq!(
            f.treasury.try_agent_create_market(
                &BytesN::from_array(&f.env, &[25; 32]),
                &BytesN::from_array(&f.env, &[125; 32]),
                &String::from_str(&f.env, "ipfs://net-2"),
                &Category::Macro,
                &MARKET_CLOSE,
                &(100 * USDC),
                &5000u32,
            ),
            Err(Ok(Error::NetExposureExceeded))
        );
    }

    #[test]
    fn sell_frees_net_headroom_but_not_lifetime_cap() {
        let f = fixture_with_net_cap(20 * USDC, 50 * USDC, 60 * USDC, 30 * USDC);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        let quote = f
            .market_core
            .quote_sell(&f.market_id, &Outcome::Yes, &(5 * USDC));
        f.treasury
            .agent_sell(&f.market_id, &Outcome::Yes, &(5 * USDC), &quote);
        assert_eq!(f.treasury.net_exposure(), 15 * USDC);
        assert_eq!(
            f.treasury.net_position(&f.market_id),
            NetPosition {
                collateral_in: 20 * USDC,
                collateral_out: 5 * USDC
            }
        );

        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(15 * USDC), &0);
        assert_eq!(f.treasury.net_exposure(), 30 * USDC);
        let exposure = f.env.as_contract(&f.treasury_id, || {
            Treasury::market_exposure(&f.env, f.market_id)
        });
        // The lifetime cap counts the sold capital too.
        assert_eq!(exposure.amount, 35 * USDC);
        assert_eq!(
            f.treasury
                .try_agent_buy(&f.market_id, &Outcome::Yes, &USDC, &0),
            Err(Ok(Error::NetExposureExceeded))
        );
        // close_trade changes neither cap.
        f.treasury.close_trade(&f.market_id);
        assert_eq!(f.treasury.net_exposure(), 30 * USDC);
    }

    #[test]
    fn redeem_and_claim_reduce_net_exposure() {
        let f = net_fixture(300 * USDC);
        let market_id = agent_market(&f, 26);
        f.treasury
            .agent_buy(&market_id, &Outcome::Yes, &(20 * USDC), &0);
        assert_eq!(f.treasury.net_exposure(), 120 * USDC);

        f.env.ledger().set_timestamp(MARKET_CLOSE);
        f.market_core.resolve(&market_id, &Outcome::Yes);
        let redeemed = f.treasury.redeem(&market_id);
        let claimed = f.treasury.claim_pool_remainder(&market_id);
        assert!(redeemed > 0 && claimed > 0);
        assert_eq!(
            f.treasury.net_position(&market_id),
            NetPosition {
                collateral_in: 120 * USDC,
                collateral_out: redeemed + claimed
            }
        );
        assert_eq!(
            f.treasury.net_exposure(),
            (120 * USDC - redeemed - claimed).max(0)
        );
    }

    #[test]
    fn profit_in_one_market_does_not_offset_another() {
        let f = net_fixture(300 * USDC);
        let other = extra_market(&f, 27);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        f.treasury
            .agent_buy(&other, &Outcome::Yes, &(20 * USDC), &0);

        f.env.ledger().set_timestamp(MARKET_CLOSE);
        f.market_core.resolve(&f.market_id, &Outcome::Yes);
        let payout = f.treasury.redeem(&f.market_id);
        assert!(payout > 20 * USDC, "winning shares pay out at a profit");
        assert_eq!(f.treasury.net_position(&f.market_id).collateral_out, payout);
        // The profitable market floors at 0; the other keeps its 20.
        assert_eq!(f.treasury.net_exposure(), 20 * USDC);
    }

    #[test]
    fn valuation_floor_counts_complete_sets_and_lp_min_reserve() {
        let f = net_fixture(300 * USDC);
        let market_id = agent_market(&f, 28);
        f.treasury
            .agent_buy(&market_id, &Outcome::Yes, &(20 * USDC), &0);
        f.treasury
            .agent_buy(&market_id, &Outcome::No, &(10 * USDC), &0);

        let position = f.market_core.get_position(&market_id, &f.treasury_id);
        let market = f.market_core.get_market(&market_id);
        let floor = position.yes.min(position.no) + market.reserve_yes.min(market.reserve_no);
        let valuation = f.treasury.market_valuation(&market_id);
        assert_eq!(valuation.cost_basis, 130 * USDC);
        assert_eq!(valuation.floor_value, floor);
        assert_eq!(valuation.at_risk, (130 * USDC - floor).max(0));
        assert!(valuation.spot_value >= valuation.floor_value);
    }

    #[test]
    fn spot_manipulation_does_not_change_enforced_headroom() {
        let f = fixture_with_net_cap(20 * USDC, 50 * USDC, 60 * USDC, 30 * USDC);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        let before = f.treasury.market_valuation(&f.market_id);

        // A third party pumps YES hard.
        let whale = Address::generate(&f.env);
        StellarAssetClient::new(&f.env, &f.token).mint(&whale, &(500 * USDC));
        f.market_core
            .buy(&whale, &f.market_id, &Outcome::Yes, &(400 * USDC), &0);
        let after = f.treasury.market_valuation(&f.market_id);
        assert!(after.spot_value > before.spot_value);
        assert_eq!(after.cost_basis, before.cost_basis);
        assert_eq!(f.treasury.net_exposure(), 20 * USDC);

        // Exactly the same 10 of headroom as before the pump.
        let other = extra_market(&f, 29);
        assert_eq!(
            f.treasury
                .try_agent_buy(&other, &Outcome::Yes, &(11 * USDC), &0),
            Err(Ok(Error::NetExposureExceeded))
        );
        f.treasury
            .agent_buy(&other, &Outcome::Yes, &(10 * USDC), &0);
    }

    #[test]
    fn valuation_after_resolution_uses_settlement_semantics() {
        let f = net_fixture(300 * USDC);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::No, &(10 * USDC), &0);
        let position = f.market_core.get_position(&f.market_id, &f.treasury_id);
        let voided = extra_market(&f, 30);
        f.treasury
            .agent_buy(&voided, &Outcome::Yes, &(20 * USDC), &0);
        let voided_position = f.market_core.get_position(&voided, &f.treasury_id);

        f.env.ledger().set_timestamp(MARKET_CLOSE);
        f.market_core.resolve(&f.market_id, &Outcome::No);
        f.market_core.void_market(&voided);

        let resolved = f.treasury.market_valuation(&f.market_id);
        assert_eq!(resolved.floor_value, position.no);
        assert_eq!(resolved.spot_value, position.no);
        let void = f.treasury.market_valuation(&voided);
        assert_eq!(
            void.floor_value,
            (voided_position.yes + voided_position.no) / 2
        );
        assert_eq!(void.spot_value, void.floor_value);
    }

    /// Stand-in market-core exposing only `buy`, which pulls the collateral
    /// like the real one and returns one share per stroop. If `agent_buy`
    /// made any other market-core call (a valuation read on the hot path),
    /// it would fail against this contract.
    #[contract]
    struct BuyOnlyMarketCore;

    #[contractimpl]
    impl BuyOnlyMarketCore {
        pub fn set_token(env: Env, token: Address) {
            env.storage().instance().set(&0u32, &token);
        }

        pub fn calls(env: Env) -> u32 {
            env.storage().instance().get(&1u32).unwrap_or(0)
        }

        pub fn buy(
            env: Env,
            trader: Address,
            _market_id: u64,
            _outcome: Outcome,
            collateral_in: i128,
            _min_shares_out: i128,
        ) -> i128 {
            trader.require_auth();
            let token: Address = env.storage().instance().get(&0u32).unwrap();
            soroban_sdk::token::TokenClient::new(&env, &token).transfer(
                &trader,
                env.current_contract_address(),
                &collateral_in,
            );
            let calls = Self::calls(env.clone());
            env.storage().instance().set(&1u32, &(calls + 1));
            collateral_in
        }
    }

    #[test]
    fn agent_buy_hot_path_makes_no_extra_cross_contract_reads() {
        let f = fixture();
        let proxy_id = f.env.register(BuyOnlyMarketCore, ());
        let proxy = BuyOnlyMarketCoreClient::new(&f.env, &proxy_id);
        proxy.set_token(&f.token);
        let config = f.treasury.get_config();
        let treasury_id = f.env.register(
            Treasury,
            TreasuryArgs::__constructor(
                &f.admin,
                &config.agent,
                &f.token,
                &proxy_id,
                &config.max_trade,
                &config.max_market,
                &config.max_daily,
                &config.max_daily,
            ),
        );
        StellarAssetClient::new(&f.env, &f.token).mint(&treasury_id, &(100 * USDC));
        let treasury = TreasuryClient::new(&f.env, &treasury_id);

        // Only the agent's call is mocked: the stub's token pull must be
        // covered by the treasury's own pre-authorization, as in production.
        f.env.set_auths(&[]);
        let (agent, amount) = (config.agent.clone(), 20 * USDC);
        f.env.cost_estimate().budget().reset_unlimited();
        let shares = treasury
            .mock_auths(&[MockAuth {
                address: &agent,
                invoke: &MockAuthInvoke {
                    contract: &treasury_id,
                    fn_name: "agent_buy",
                    args: (f.market_id, Outcome::Yes, amount, 0_i128).into_val(&f.env),
                    sub_invokes: &[],
                },
            }])
            .agent_buy(&f.market_id, &Outcome::Yes, &amount, &0);
        let used = f.env.cost_estimate().resources();
        std::println!(
            "agent_buy with net accounting: {} instructions, {} reads, {} writes",
            used.instructions,
            used.disk_read_entries,
            used.write_entries
        );
        assert!(shares > 0);
        assert_eq!(proxy.calls(), 1);
        assert_eq!(treasury.net_exposure(), 20 * USDC);
    }

    #[test]
    #[should_panic(expected = "invalid risk configuration")]
    fn constructor_rejects_net_cap_below_max_trade() {
        fixture_with_net_cap(20 * USDC, 50 * USDC, 60 * USDC, 20 * USDC - 1);
    }

    #[test]
    fn migrate_market_clears_net_exposure() {
        let f = net_fixture(300 * USDC);
        f.treasury
            .agent_buy(&f.market_id, &Outcome::Yes, &(20 * USDC), &0);
        f.treasury.set_successor(&Address::generate(&f.env));
        f.treasury.migrate_market(&f.market_id);
        assert_eq!(f.treasury.net_exposure(), 0);
    }
}
