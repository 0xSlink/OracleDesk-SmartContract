import { Buffer } from "buffer";
import { Address } from "@stellar/stellar-sdk";
import {
  AssembledTransaction,
  Client as ContractClient,
  ClientOptions as ContractClientOptions,
  MethodOptions,
  Result,
  Spec as ContractSpec,
} from "@stellar/stellar-sdk/contract";
import type {
  u32,
  i32,
  u64,
  i64,
  u128,
  i128,
  u256,
  i256,
  Option,
  Timepoint,
  Duration,
} from "@stellar/stellar-sdk/contract";
export * from "@stellar/stellar-sdk";
export * as contract from "@stellar/stellar-sdk/contract";
export * as rpc from "@stellar/stellar-sdk/rpc";

if (typeof window !== "undefined") {
  //@ts-ignore Buffer exists
  window.Buffer = window.Buffer || Buffer;
}




export type Outcome = {tag: "Yes", values: void} | {tag: "No", values: void};

export type Category = {tag: "Crypto", values: void} | {tag: "Macro", values: void} | {tag: "Geopolitics", values: void} | {tag: "Sports", values: void} | {tag: "Culture", values: void} | {tag: "Other", values: void};

export const Errors = {
  1: {message:"NotInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"InvalidConfig"},
  4: {message:"InvalidAmount"},
  5: {message:"TradeCapExceeded"},
  6: {message:"MarketCapExceeded"},
  7: {message:"DailyCapExceeded"},
  8: {message:"MarketNotOpen"},
  9: {message:"InsufficientBalance"},
  10: {message:"Paused"},
  11: {message:"NoSuccessor"},
  /**
   * Not market-core's error 12 (`SlippageExceeded`): market-core errors
   * reach callers of treasury entrypoints with market-core's own codes.
   */
  12: {message:"NetExposureExceeded"}
}


/**
 * `market_valuation` result. Only `cost_basis` is enforced (by
 * `max_net_exposure`); the others are informational.
 */
export interface Valuation {
  /**
 * `max(0, cost_basis - floor_value)`.
 */
at_risk: i128;
  cost_basis: i128;
  /**
 * What the treasury gets back whatever the outcome, from state a price
 * move cannot raise: complete sets `min(yes, no)` plus, as the unclaimed
 * LP, `min(reserve_yes, reserve_no)`. After finality, the settlement
 * value.
 */
floor_value: i128;
  /**
 * Holdings at the pool's spot price. Manipulable by any trader, so it is
 * never used for enforcement.
 */
spot_value: i128;
}


/**
 * Summary of the sliding daily window, returned by `daily_usage`.
 * `day_start` is the start of the oldest hour still counted and `amount`
 * the usage summed over the whole window (see [`UsageWindow`]).
 */
export interface DailyUsage {
  amount: i128;
  day_start: u64;
}


export interface RiskConfig {
  agent: string;
  collateral: string;
  market_core: string;
  max_daily: i128;
  max_market: i128;
  max_trade: i128;
}


/**
 * Collateral the treasury has sent into (`collateral_in`) and received back
 * from (`collateral_out`) one market. Accounting starts when this code was
 * deployed: a missing entry reads as zero.
 */
export interface NetPosition {
  collateral_in: i128;
  collateral_out: i128;
}


/**
 * Hourly ring buffer behind `max_daily`. Kept in instance storage: ~400
 * bytes that every call loads anyway, so it costs no extra ledger-entry
 * read (a persistent entry would add one per trade).
 */
export interface UsageWindow {
  /**
 * Always `BUCKETS` long. `amounts[i]` is the usage of hour
 * `head_hour - ((head_idx - i) mod BUCKETS)`.
 */
amounts: Array<i128>;
  /**
 * Hour index (`timestamp / BUCKET_SECONDS`) of the newest bucket.
 */
head_hour: u64;
  head_idx: u32;
}







export interface Client {
  /**
   * Construct and simulate a redeem transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Redeems the treasury's position in a resolved/voided market.
   */
  redeem: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a deposit transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  deposit: ({from, amount}: {from: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  withdraw: ({to, amount}: {to: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a agent_buy transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Buys `outcome` shares in an existing market using treasury capital.
   */
  agent_buy: ({market_id, outcome, collateral_in, min_shares_out}: {market_id: u64, outcome: Outcome, collateral_in: i128, min_shares_out: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a agent_sell transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sells `outcome` shares back into the pool for exactly
   * `collateral_out`. This returns capital to the treasury (market-core
   * pays out of its own balance), so it is not capped and does not need
   * the nested-transfer pre-authorization that buying does. It frees net
   * headroom (`collateral_out` is recorded) but never lifetime headroom.
   */
  agent_sell: ({market_id, outcome, collateral_out, max_shares_in}: {market_id: u64, outcome: Outcome, collateral_out: i128, max_shares_in: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_config: (options?: MethodOptions) => Promise<AssembledTransaction<Result<RiskConfig>>>

  /**
   * Construct and simulate a set_paused transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_paused: ({paused}: {paused: boolean}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a close_trade transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Marks a market's exposure as exited. Pure bookkeeping (see
   * `MarketExposure`) for the future exposure dashboard; it does not move
   * funds or free either the cumulative per-market cap or net headroom.
   */
  close_trade: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a daily_usage transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Usage over the sliding window as of now: `amount` sums every bucket
   * still counted against `max_daily`, `day_start` is the start of the
   * oldest of them. Side-effect free (it advances a copy), so repeated
   * reads without trades in between return the same value.
   */
  daily_usage: (options?: MethodOptions) => Promise<AssembledTransaction<Result<DailyUsage>>>

  /**
   * Construct and simulate a get_net_cap transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * `max_net_exposure`. A treasury deployed before the net cap existed has
   * no stored value and uses `max_daily`.
   */
  get_net_cap: (options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a agent_buy_v2 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * `agent_buy` with a per-share limit price (`limit_price_bps`, 0 = none)
   * and a deadline (unix seconds, network time). market-core's `buy_v2`
   * enforces both; an expired or out-of-limit trade reverts the whole
   * invocation, so the treasury's caps are never consumed by a reverted
   * trade.
   */
  agent_buy_v2: ({market_id, outcome, collateral_in, min_shares_out, limit_price_bps, deadline}: {market_id: u64, outcome: Outcome, collateral_in: i128, min_shares_out: i128, limit_price_bps: u32, deadline: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a net_exposure transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Capital currently deployed across all markets:
   * `sum(max(0, collateral_in - collateral_out))`, the figure
   * `max_net_exposure` is enforced against.
   */
  net_exposure: (options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a net_position transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  net_position: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<NetPosition>>

  /**
   * Construct and simulate a agent_sell_v2 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * `agent_sell` with a per-share limit price (`limit_price_bps`, 0 = no
   * limit) and a deadline (unix seconds, network time), both enforced by
   * market-core's `sell_v2`.
   */
  agent_sell_v2: ({market_id, outcome, collateral_out, max_shares_in, limit_price_bps, deadline}: {market_id: u64, outcome: Outcome, collateral_out: i128, max_shares_in: i128, limit_price_bps: u32, deadline: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a set_successor transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Sets the successor treasury that `migrate_market` hands positions and
   * the LP role to. Admin-only; overwrites any previous successor (the
   * `SuccessorSet` events are the audit trail).
   */
  set_successor: ({successor}: {successor: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a daily_headroom transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * How much more the agent may deploy right now before `max_daily`
   * (`max_daily - window sum`, floored at 0), for pre-flight checks.
   */
  daily_headroom: (options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a migrate_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Moves the treasury's position in `market_id` — and the LP role, when
   * the treasury is the market's creator and has not claimed the remainder
   * yet — to the successor set via `set_successor`, then closes the local
   * exposure. Admin-only: relocating capital is not an agent trading
   * action, and the agent key must not be able to drain positions.
   * 
   * market-core's `transfer_position` / `transfer_lp` require the treasury
   * itself to authorize. Because the treasury is their *direct* caller
   * here, the implicit invoker authorization covers both — no
   * `authorize_as_current_contract` entry is needed (unlike the nested
   * token transfer in `agent_buy`, see `docs/adr/0002-treasury-nested-auth.md`).
   */
  migrate_market: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a market_valuation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Conservative valuation of the treasury's holdings in `market_id`
   * (see [`Valuation`]). Two cross-contract reads (`get_market`,
   * `get_position`); a view only, never on the trading path.
   */
  market_valuation: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Valuation>>>

  /**
   * Construct and simulate a available_capital transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  available_capital: (options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a daily_usage_detail transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * The hourly buckets behind `daily_usage`, advanced to now (not stored).
   */
  daily_usage_detail: (options?: MethodOptions) => Promise<AssembledTransaction<Result<UsageWindow>>>

  /**
   * Construct and simulate a agent_claim_lp_fees transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claims the market's LP fee share for a market the treasury created.
   * The LP portion accrues separately on market-core (see its `claim_lp_fees`);
   * this is that claim, agent-gated like the other trading entrypoints.
   */
  agent_claim_lp_fees: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a agent_create_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Creates and seeds a market using treasury capital. The treasury
   * becomes the market's creator/LP.
   */
  agent_create_market: ({question_hash, resolution_hash, meta_uri, category, close_time, seed_amount, initial_yes_bps}: {question_hash: Buffer, resolution_hash: Buffer, meta_uri: string, category: Category, close_time: u64, seed_amount: i128, initial_yes_bps: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a collect_market_fees transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pulls accrued **protocol** fees from a configured market-core contract.
   * 
   * Only the protocol's share of each market's fees: the LP portion accrues
   * separately on market-core and belongs to the market's creator — for
   * agent-created markets that is the treasury itself, via
   * `agent_claim_lp_fees`.
   */
  collect_market_fees: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a claim_pool_remainder transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Claims the LP pool remainder for a market the treasury created.
   */
  claim_pool_remainder: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {admin, agent, collateral, market_core, max_trade, max_market, max_daily, max_net_exposure}: {admin: string, agent: string, collateral: string, market_core: string, max_trade: i128, max_market: i128, max_daily: i128, max_net_exposure: i128},
    /** Options for initializing a Client as well as for calling a method, with extras specific to deploying. */
    options: MethodOptions &
      Omit<ContractClientOptions, "contractId"> & {
        /** The hash of the Wasm blob, which must already be installed on-chain. */
        wasmHash: Buffer | string;
        /** Salt used to generate the contract's ID. Passed through to {@link Operation.createCustomContract}. Default: random. */
        salt?: Buffer | Uint8Array;
        /** The format used to decode `wasmHash`, if it's provided as a string. */
        format?: "hex" | "base64";
      }
  ): Promise<AssembledTransaction<T>> {
    return ContractClient.deploy({admin, agent, collateral, market_core, max_trade, max_market, max_daily, max_net_exposure}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAgAAAAAAAAAAAAAAB091dGNvbWUAAAAAAgAAAAAAAAAAAAAAA1llcwAAAAAAAAAAAAAAAAJObwAA",
        "AAAAAgAAAAAAAAAAAAAACENhdGVnb3J5AAAABgAAAAAAAAAAAAAABkNyeXB0bwAAAAAAAAAAAAAAAAAFTWFjcm8AAAAAAAAAAAAAAAAAAAtHZW9wb2xpdGljcwAAAAAAAAAAAAAAAAZTcG9ydHMAAAAAAAAAAAAAAAAAB0N1bHR1cmUAAAAAAAAAAAAAAAAFT3RoZXIAAAA=",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAADAAAAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADFVuYXV0aG9yaXplZAAAAAIAAAAAAAAADUludmFsaWRDb25maWcAAAAAAAADAAAAAAAAAA1JbnZhbGlkQW1vdW50AAAAAAAABAAAAAAAAAAQVHJhZGVDYXBFeGNlZWRlZAAAAAUAAAAAAAAAEU1hcmtldENhcEV4Y2VlZGVkAAAAAAAABgAAAAAAAAAQRGFpbHlDYXBFeGNlZWRlZAAAAAcAAAAAAAAADU1hcmtldE5vdE9wZW4AAAAAAAAIAAAAAAAAABNJbnN1ZmZpY2llbnRCYWxhbmNlAAAAAAkAAAAAAAAABlBhdXNlZAAAAAAACgAAAAAAAAALTm9TdWNjZXNzb3IAAAAACwAAAIdOb3QgbWFya2V0LWNvcmUncyBlcnJvciAxMiAoYFNsaXBwYWdlRXhjZWVkZWRgKTogbWFya2V0LWNvcmUgZXJyb3JzCnJlYWNoIGNhbGxlcnMgb2YgdHJlYXN1cnkgZW50cnlwb2ludHMgd2l0aCBtYXJrZXQtY29yZSdzIG93biBjb2Rlcy4AAAAAE05ldEV4cG9zdXJlRXhjZWVkZWQAAAAADA==",
        "AAAAAAAAADxSZWRlZW1zIHRoZSB0cmVhc3VyeSdzIHBvc2l0aW9uIGluIGEgcmVzb2x2ZWQvdm9pZGVkIG1hcmtldC4AAAAGcmVkZWVtAAAAAAABAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAA+kAAAALAAAAAw==",
        "AAAAAQAAAG9gbWFya2V0X3ZhbHVhdGlvbmAgcmVzdWx0LiBPbmx5IGBjb3N0X2Jhc2lzYCBpcyBlbmZvcmNlZCAoYnkKYG1heF9uZXRfZXhwb3N1cmVgKTsgdGhlIG90aGVycyBhcmUgaW5mb3JtYXRpb25hbC4AAAAAAAAAAAlWYWx1YXRpb24AAAAAAAAEAAAAI2BtYXgoMCwgY29zdF9iYXNpcyAtIGZsb29yX3ZhbHVlKWAuAAAAAAdhdF9yaXNrAAAAAAsAAAAAAAAACmNvc3RfYmFzaXMAAAAAAAsAAADVV2hhdCB0aGUgdHJlYXN1cnkgZ2V0cyBiYWNrIHdoYXRldmVyIHRoZSBvdXRjb21lLCBmcm9tIHN0YXRlIGEgcHJpY2UKbW92ZSBjYW5ub3QgcmFpc2U6IGNvbXBsZXRlIHNldHMgYG1pbih5ZXMsIG5vKWAgcGx1cywgYXMgdGhlIHVuY2xhaW1lZApMUCwgYG1pbihyZXNlcnZlX3llcywgcmVzZXJ2ZV9ubylgLiBBZnRlciBmaW5hbGl0eSwgdGhlIHNldHRsZW1lbnQKdmFsdWUuAAAAAAAAC2Zsb29yX3ZhbHVlAAAAAAsAAABiSG9sZGluZ3MgYXQgdGhlIHBvb2wncyBzcG90IHByaWNlLiBNYW5pcHVsYWJsZSBieSBhbnkgdHJhZGVyLCBzbyBpdCBpcwpuZXZlciB1c2VkIGZvciBlbmZvcmNlbWVudC4AAAAAAApzcG90X3ZhbHVlAAAAAAAL",
        "AAAAAAAAAAAAAAAHZGVwb3NpdAAAAAACAAAAAAAAAARmcm9tAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAQAAAMRTdW1tYXJ5IG9mIHRoZSBzbGlkaW5nIGRhaWx5IHdpbmRvdywgcmV0dXJuZWQgYnkgYGRhaWx5X3VzYWdlYC4KYGRheV9zdGFydGAgaXMgdGhlIHN0YXJ0IG9mIHRoZSBvbGRlc3QgaG91ciBzdGlsbCBjb3VudGVkIGFuZCBgYW1vdW50YAp0aGUgdXNhZ2Ugc3VtbWVkIG92ZXIgdGhlIHdob2xlIHdpbmRvdyAoc2VlIFtgVXNhZ2VXaW5kb3dgXSkuAAAAAAAAAApEYWlseVVzYWdlAAAAAAACAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAACWRheV9zdGFydAAAAAAAAAY=",
        "AAAAAQAAAAAAAAAAAAAAClJpc2tDb25maWcAAAAAAAYAAAAAAAAABWFnZW50AAAAAAAAEwAAAAAAAAAKY29sbGF0ZXJhbAAAAAAAEwAAAAAAAAALbWFya2V0X2NvcmUAAAAAEwAAAAAAAAAJbWF4X2RhaWx5AAAAAAAACwAAAAAAAAAKbWF4X21hcmtldAAAAAAACwAAAAAAAAAJbWF4X3RyYWRlAAAAAAAACw==",
        "AAAAAAAAAAAAAAAId2l0aGRyYXcAAAACAAAAAAAAAAJ0bwAAAAAAEwAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAQAAALtDb2xsYXRlcmFsIHRoZSB0cmVhc3VyeSBoYXMgc2VudCBpbnRvIChgY29sbGF0ZXJhbF9pbmApIGFuZCByZWNlaXZlZCBiYWNrCmZyb20gKGBjb2xsYXRlcmFsX291dGApIG9uZSBtYXJrZXQuIEFjY291bnRpbmcgc3RhcnRzIHdoZW4gdGhpcyBjb2RlIHdhcwpkZXBsb3llZDogYSBtaXNzaW5nIGVudHJ5IHJlYWRzIGFzIHplcm8uAAAAAAAAAAALTmV0UG9zaXRpb24AAAAAAgAAAAAAAAANY29sbGF0ZXJhbF9pbgAAAAAAAAsAAAAAAAAADmNvbGxhdGVyYWxfb3V0AAAAAAAL",
        "AAAAAQAAAL5Ib3VybHkgcmluZyBidWZmZXIgYmVoaW5kIGBtYXhfZGFpbHlgLiBLZXB0IGluIGluc3RhbmNlIHN0b3JhZ2U6IH40MDAKYnl0ZXMgdGhhdCBldmVyeSBjYWxsIGxvYWRzIGFueXdheSwgc28gaXQgY29zdHMgbm8gZXh0cmEgbGVkZ2VyLWVudHJ5CnJlYWQgKGEgcGVyc2lzdGVudCBlbnRyeSB3b3VsZCBhZGQgb25lIHBlciB0cmFkZSkuAAAAAAAAAAAAC1VzYWdlV2luZG93AAAAAAMAAABkQWx3YXlzIGBCVUNLRVRTYCBsb25nLiBgYW1vdW50c1tpXWAgaXMgdGhlIHVzYWdlIG9mIGhvdXIKYGhlYWRfaG91ciAtICgoaGVhZF9pZHggLSBpKSBtb2QgQlVDS0VUUylgLgAAAAdhbW91bnRzAAAAA+oAAAALAAAAP0hvdXIgaW5kZXggKGB0aW1lc3RhbXAgLyBCVUNLRVRfU0VDT05EU2ApIG9mIHRoZSBuZXdlc3QgYnVja2V0LgAAAAAJaGVhZF9ob3VyAAAAAAAABgAAAAAAAAAIaGVhZF9pZHgAAAAE",
        "AAAAAAAAAENCdXlzIGBvdXRjb21lYCBzaGFyZXMgaW4gYW4gZXhpc3RpbmcgbWFya2V0IHVzaW5nIHRyZWFzdXJ5IGNhcGl0YWwuAAAAAAlhZ2VudF9idXkAAAAAAAAEAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAAAAAAdvdXRjb21lAAAAB9AAAAAHT3V0Y29tZQAAAAAAAAAADWNvbGxhdGVyYWxfaW4AAAAAAAALAAAAAAAAAA5taW5fc2hhcmVzX291dAAAAAAACwAAAAEAAAPpAAAACwAAAAM=",
        "AAAAAAAAAUdTZWxscyBgb3V0Y29tZWAgc2hhcmVzIGJhY2sgaW50byB0aGUgcG9vbCBmb3IgZXhhY3RseQpgY29sbGF0ZXJhbF9vdXRgLiBUaGlzIHJldHVybnMgY2FwaXRhbCB0byB0aGUgdHJlYXN1cnkgKG1hcmtldC1jb3JlCnBheXMgb3V0IG9mIGl0cyBvd24gYmFsYW5jZSksIHNvIGl0IGlzIG5vdCBjYXBwZWQgYW5kIGRvZXMgbm90IG5lZWQKdGhlIG5lc3RlZC10cmFuc2ZlciBwcmUtYXV0aG9yaXphdGlvbiB0aGF0IGJ1eWluZyBkb2VzLiBJdCBmcmVlcyBuZXQKaGVhZHJvb20gKGBjb2xsYXRlcmFsX291dGAgaXMgcmVjb3JkZWQpIGJ1dCBuZXZlciBsaWZldGltZSBoZWFkcm9vbS4AAAAACmFnZW50X3NlbGwAAAAAAAQAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAAB291dGNvbWUAAAAH0AAAAAdPdXRjb21lAAAAAAAAAAAOY29sbGF0ZXJhbF9vdXQAAAAAAAsAAAAAAAAADW1heF9zaGFyZXNfaW4AAAAAAAALAAAAAQAAA+kAAAALAAAAAw==",
        "AAAAAAAAAAAAAAAKZ2V0X2NvbmZpZwAAAAAAAAAAAAEAAAPpAAAH0AAAAApSaXNrQ29uZmlnAAAAAAAD",
        "AAAAAAAAAAAAAAAKc2V0X3BhdXNlZAAAAAAAAQAAAAAAAAAGcGF1c2VkAAAAAAABAAAAAQAAA+kAAAACAAAAAw==",
        "AAAABQAAAAAAAAAAAAAADFN1Y2Nlc3NvclNldAAAAAEAAAANc3VjY2Vzc29yX3NldAAAAAAAAAIAAAAAAAAAA29sZAAAAAPoAAAAEwAAAAAAAAAAAAAAA25ldwAAAAATAAAAAAAAAAI=",
        "AAAAAAAAAMRNYXJrcyBhIG1hcmtldCdzIGV4cG9zdXJlIGFzIGV4aXRlZC4gUHVyZSBib29ra2VlcGluZyAoc2VlCmBNYXJrZXRFeHBvc3VyZWApIGZvciB0aGUgZnV0dXJlIGV4cG9zdXJlIGRhc2hib2FyZDsgaXQgZG9lcyBub3QgbW92ZQpmdW5kcyBvciBmcmVlIGVpdGhlciB0aGUgY3VtdWxhdGl2ZSBwZXItbWFya2V0IGNhcCBvciBuZXQgaGVhZHJvb20uAAAAC2Nsb3NlX3RyYWRlAAAAAAEAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAD6QAAAAsAAAAD",
        "AAAAAAAAAQBVc2FnZSBvdmVyIHRoZSBzbGlkaW5nIHdpbmRvdyBhcyBvZiBub3c6IGBhbW91bnRgIHN1bXMgZXZlcnkgYnVja2V0CnN0aWxsIGNvdW50ZWQgYWdhaW5zdCBgbWF4X2RhaWx5YCwgYGRheV9zdGFydGAgaXMgdGhlIHN0YXJ0IG9mIHRoZQpvbGRlc3Qgb2YgdGhlbS4gU2lkZS1lZmZlY3QgZnJlZSAoaXQgYWR2YW5jZXMgYSBjb3B5KSwgc28gcmVwZWF0ZWQKcmVhZHMgd2l0aG91dCB0cmFkZXMgaW4gYmV0d2VlbiByZXR1cm4gdGhlIHNhbWUgdmFsdWUuAAAAC2RhaWx5X3VzYWdlAAAAAAAAAAABAAAD6QAAB9AAAAAKRGFpbHlVc2FnZQAAAAAAAw==",
        "AAAAAAAAAGxgbWF4X25ldF9leHBvc3VyZWAuIEEgdHJlYXN1cnkgZGVwbG95ZWQgYmVmb3JlIHRoZSBuZXQgY2FwIGV4aXN0ZWQgaGFzCm5vIHN0b3JlZCB2YWx1ZSBhbmQgdXNlcyBgbWF4X2RhaWx5YC4AAAALZ2V0X25ldF9jYXAAAAAAAAAAAAEAAAPpAAAACwAAAAM=",
        "AAAABQAAAAAAAAAAAAAADUZlZXNDb2xsZWN0ZWQAAAAAAAABAAAADmZlZXNfY29sbGVjdGVkAAAAAAACAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAI=",
        "AAAAAAAAARdgYWdlbnRfYnV5YCB3aXRoIGEgcGVyLXNoYXJlIGxpbWl0IHByaWNlIChgbGltaXRfcHJpY2VfYnBzYCwgMCA9IG5vbmUpCmFuZCBhIGRlYWRsaW5lICh1bml4IHNlY29uZHMsIG5ldHdvcmsgdGltZSkuIG1hcmtldC1jb3JlJ3MgYGJ1eV92MmAKZW5mb3JjZXMgYm90aDsgYW4gZXhwaXJlZCBvciBvdXQtb2YtbGltaXQgdHJhZGUgcmV2ZXJ0cyB0aGUgd2hvbGUKaW52b2NhdGlvbiwgc28gdGhlIHRyZWFzdXJ5J3MgY2FwcyBhcmUgbmV2ZXIgY29uc3VtZWQgYnkgYSByZXZlcnRlZAp0cmFkZS4AAAAADGFnZW50X2J1eV92MgAAAAYAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAAB291dGNvbWUAAAAH0AAAAAdPdXRjb21lAAAAAAAAAAANY29sbGF0ZXJhbF9pbgAAAAAAAAsAAAAAAAAADm1pbl9zaGFyZXNfb3V0AAAAAAALAAAAAAAAAA9saW1pdF9wcmljZV9icHMAAAAABAAAAAAAAAAIZGVhZGxpbmUAAAAGAAAAAQAAA+kAAAALAAAAAw==",
        "AAAAAAAAAJBDYXBpdGFsIGN1cnJlbnRseSBkZXBsb3llZCBhY3Jvc3MgYWxsIG1hcmtldHM6CmBzdW0obWF4KDAsIGNvbGxhdGVyYWxfaW4gLSBjb2xsYXRlcmFsX291dCkpYCwgdGhlIGZpZ3VyZQpgbWF4X25ldF9leHBvc3VyZWAgaXMgZW5mb3JjZWQgYWdhaW5zdC4AAAAMbmV0X2V4cG9zdXJlAAAAAAAAAAEAAAAL",
        "AAAAAAAAAAAAAAAMbmV0X3Bvc2l0aW9uAAAAAQAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAEAAAfQAAAAC05ldFBvc2l0aW9uAA==",
        "AAAABQAAAAAAAAAAAAAADk1hcmtldE1pZ3JhdGVkAAAAAAABAAAAD21hcmtldF9taWdyYXRlZAAAAAAFAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAJc3VjY2Vzc29yAAAAAAAAEwAAAAEAAAAAAAAAA3llcwAAAAALAAAAAAAAAAAAAAACbm8AAAAAAAsAAAAAAAAAAAAAAAJscAAAAAAAAQAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAADlBvc2l0aW9uQ2xvc2VkAAAAAAABAAAAD3Bvc2l0aW9uX2Nsb3NlZAAAAAACAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAI=",
        "AAAAAAAAAIxgbWF4X25ldF9leHBvc3VyZWAgY2FwcyBjYXBpdGFsIGRlcGxveWVkIGluIG9wZW4gcG9zaXRpb25zIGFjcm9zcyBhbGwKbWFya2V0cyBhdCBvbmNlIChzZWUgYG5ldF9leHBvc3VyZWApOyBpdCBtdXN0IGJlIGF0IGxlYXN0IGBtYXhfdHJhZGVgLgAAAA1fX2NvbnN0cnVjdG9yAAAAAAAACAAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAAAVhZ2VudAAAAAAAABMAAAAAAAAACmNvbGxhdGVyYWwAAAAAABMAAAAAAAAAC21hcmtldF9jb3JlAAAAABMAAAAAAAAACW1heF90cmFkZQAAAAAAAAsAAAAAAAAACm1heF9tYXJrZXQAAAAAAAsAAAAAAAAACW1heF9kYWlseQAAAAAAAAsAAAAAAAAAEG1heF9uZXRfZXhwb3N1cmUAAAALAAAAAA==",
        "AAAAAAAAAKJgYWdlbnRfc2VsbGAgd2l0aCBhIHBlci1zaGFyZSBsaW1pdCBwcmljZSAoYGxpbWl0X3ByaWNlX2Jwc2AsIDAgPSBubwpsaW1pdCkgYW5kIGEgZGVhZGxpbmUgKHVuaXggc2Vjb25kcywgbmV0d29yayB0aW1lKSwgYm90aCBlbmZvcmNlZCBieQptYXJrZXQtY29yZSdzIGBzZWxsX3YyYC4AAAAAAA1hZ2VudF9zZWxsX3YyAAAAAAAABgAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAAAAAAHb3V0Y29tZQAAAAfQAAAAB091dGNvbWUAAAAAAAAAAA5jb2xsYXRlcmFsX291dAAAAAAACwAAAAAAAAANbWF4X3NoYXJlc19pbgAAAAAAAAsAAAAAAAAAD2xpbWl0X3ByaWNlX2JwcwAAAAAEAAAAAAAAAAhkZWFkbGluZQAAAAYAAAABAAAD6QAAAAsAAAAD",
        "AAAAAAAAALRTZXRzIHRoZSBzdWNjZXNzb3IgdHJlYXN1cnkgdGhhdCBgbWlncmF0ZV9tYXJrZXRgIGhhbmRzIHBvc2l0aW9ucyBhbmQKdGhlIExQIHJvbGUgdG8uIEFkbWluLW9ubHk7IG92ZXJ3cml0ZXMgYW55IHByZXZpb3VzIHN1Y2Nlc3NvciAodGhlCmBTdWNjZXNzb3JTZXRgIGV2ZW50cyBhcmUgdGhlIGF1ZGl0IHRyYWlsKS4AAAANc2V0X3N1Y2Nlc3NvcgAAAAAAAAEAAAAAAAAACXN1Y2Nlc3NvcgAAAAAAABMAAAABAAAD6QAAAAIAAAAD",
        "AAAABQAAAAAAAAAAAAAAD1RyYWRlQXV0aG9yaXplZAAAAAABAAAAEHRyYWRlX2F1dGhvcml6ZWQAAAAFAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAFYWdlbnQAAAAAAAATAAAAAQAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAAAAAAMbWFya2V0X3RvdGFsAAAACwAAAAAAAAAAAAAAC2RhaWx5X3RvdGFsAAAAAAsAAAAAAAAAAg==",
        "AAAAAAAAAIBIb3cgbXVjaCBtb3JlIHRoZSBhZ2VudCBtYXkgZGVwbG95IHJpZ2h0IG5vdyBiZWZvcmUgYG1heF9kYWlseWAKKGBtYXhfZGFpbHkgLSB3aW5kb3cgc3VtYCwgZmxvb3JlZCBhdCAwKSwgZm9yIHByZS1mbGlnaHQgY2hlY2tzLgAAAA5kYWlseV9oZWFkcm9vbQAAAAAAAAAAAAEAAAPpAAAACwAAAAM=",
        "AAAAAAAAAqxNb3ZlcyB0aGUgdHJlYXN1cnkncyBwb3NpdGlvbiBpbiBgbWFya2V0X2lkYCDigJQgYW5kIHRoZSBMUCByb2xlLCB3aGVuCnRoZSB0cmVhc3VyeSBpcyB0aGUgbWFya2V0J3MgY3JlYXRvciBhbmQgaGFzIG5vdCBjbGFpbWVkIHRoZSByZW1haW5kZXIKeWV0IOKAlCB0byB0aGUgc3VjY2Vzc29yIHNldCB2aWEgYHNldF9zdWNjZXNzb3JgLCB0aGVuIGNsb3NlcyB0aGUgbG9jYWwKZXhwb3N1cmUuIEFkbWluLW9ubHk6IHJlbG9jYXRpbmcgY2FwaXRhbCBpcyBub3QgYW4gYWdlbnQgdHJhZGluZwphY3Rpb24sIGFuZCB0aGUgYWdlbnQga2V5IG11c3Qgbm90IGJlIGFibGUgdG8gZHJhaW4gcG9zaXRpb25zLgoKbWFya2V0LWNvcmUncyBgdHJhbnNmZXJfcG9zaXRpb25gIC8gYHRyYW5zZmVyX2xwYCByZXF1aXJlIHRoZSB0cmVhc3VyeQppdHNlbGYgdG8gYXV0aG9yaXplLiBCZWNhdXNlIHRoZSB0cmVhc3VyeSBpcyB0aGVpciAqZGlyZWN0KiBjYWxsZXIKaGVyZSwgdGhlIGltcGxpY2l0IGludm9rZXIgYXV0aG9yaXphdGlvbiBjb3ZlcnMgYm90aCDigJQgbm8KYGF1dGhvcml6ZV9hc19jdXJyZW50X2NvbnRyYWN0YCBlbnRyeSBpcyBuZWVkZWQgKHVubGlrZSB0aGUgbmVzdGVkCnRva2VuIHRyYW5zZmVyIGluIGBhZ2VudF9idXlgLCBzZWUgYGRvY3MvYWRyLzAwMDItdHJlYXN1cnktbmVzdGVkLWF1dGgubWRgKS4AAAAObWlncmF0ZV9tYXJrZXQAAAAAAAEAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAALZDb25zZXJ2YXRpdmUgdmFsdWF0aW9uIG9mIHRoZSB0cmVhc3VyeSdzIGhvbGRpbmdzIGluIGBtYXJrZXRfaWRgCihzZWUgW2BWYWx1YXRpb25gXSkuIFR3byBjcm9zcy1jb250cmFjdCByZWFkcyAoYGdldF9tYXJrZXRgLApgZ2V0X3Bvc2l0aW9uYCk7IGEgdmlldyBvbmx5LCBuZXZlciBvbiB0aGUgdHJhZGluZyBwYXRoLgAAAAAAEG1hcmtldF92YWx1YXRpb24AAAABAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAA+kAAAfQAAAACVZhbHVhdGlvbgAAAAAAAAM=",
        "AAAABQAAAD5OZXQgZXhwb3N1cmUgYWZ0ZXIgYW55IHRyZWFzdXJ5IG1vbmV5IG1vdmVtZW50IGluIGBtYXJrZXRfaWRgLgAAAAAAAAAAABJOZXRFeHBvc3VyZVVwZGF0ZWQAAAAAAAEAAAAUbmV0X2V4cG9zdXJlX3VwZGF0ZWQAAAADAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAKbmV0X21hcmtldAAAAAAACwAAAAAAAAAAAAAACW5ldF90b3RhbAAAAAAAAAsAAAAAAAAAAg==",
        "AAAAAAAAAAAAAAARYXZhaWxhYmxlX2NhcGl0YWwAAAAAAAAAAAAAAQAAA+kAAAALAAAAAw==",
        "AAAAAAAAAEZUaGUgaG91cmx5IGJ1Y2tldHMgYmVoaW5kIGBkYWlseV91c2FnZWAsIGFkdmFuY2VkIHRvIG5vdyAobm90IHN0b3JlZCkuAAAAAAASZGFpbHlfdXNhZ2VfZGV0YWlsAAAAAAAAAAAAAQAAA+kAAAfQAAAAC1VzYWdlV2luZG93AAAAAAM=",
        "AAAAAAAAANNDbGFpbXMgdGhlIG1hcmtldCdzIExQIGZlZSBzaGFyZSBmb3IgYSBtYXJrZXQgdGhlIHRyZWFzdXJ5IGNyZWF0ZWQuClRoZSBMUCBwb3J0aW9uIGFjY3J1ZXMgc2VwYXJhdGVseSBvbiBtYXJrZXQtY29yZSAoc2VlIGl0cyBgY2xhaW1fbHBfZmVlc2ApOwp0aGlzIGlzIHRoYXQgY2xhaW0sIGFnZW50LWdhdGVkIGxpa2UgdGhlIG90aGVyIHRyYWRpbmcgZW50cnlwb2ludHMuAAAAABNhZ2VudF9jbGFpbV9scF9mZWVzAAAAAAEAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAD6QAAAAsAAAAD",
        "AAAAAAAAAGBDcmVhdGVzIGFuZCBzZWVkcyBhIG1hcmtldCB1c2luZyB0cmVhc3VyeSBjYXBpdGFsLiBUaGUgdHJlYXN1cnkKYmVjb21lcyB0aGUgbWFya2V0J3MgY3JlYXRvci9MUC4AAAATYWdlbnRfY3JlYXRlX21hcmtldAAAAAAHAAAAAAAAAA1xdWVzdGlvbl9oYXNoAAAAAAAD7gAAACAAAAAAAAAAD3Jlc29sdXRpb25faGFzaAAAAAPuAAAAIAAAAAAAAAAIbWV0YV91cmkAAAAQAAAAAAAAAAhjYXRlZ29yeQAAB9AAAAAIQ2F0ZWdvcnkAAAAAAAAACmNsb3NlX3RpbWUAAAAAAAYAAAAAAAAAC3NlZWRfYW1vdW50AAAAAAsAAAAAAAAAD2luaXRpYWxfeWVzX2JwcwAAAAAEAAAAAQAAA+kAAAAGAAAAAw==",
        "AAAAAAAAASRQdWxscyBhY2NydWVkICoqcHJvdG9jb2wqKiBmZWVzIGZyb20gYSBjb25maWd1cmVkIG1hcmtldC1jb3JlIGNvbnRyYWN0LgoKT25seSB0aGUgcHJvdG9jb2wncyBzaGFyZSBvZiBlYWNoIG1hcmtldCdzIGZlZXM6IHRoZSBMUCBwb3J0aW9uIGFjY3J1ZXMKc2VwYXJhdGVseSBvbiBtYXJrZXQtY29yZSBhbmQgYmVsb25ncyB0byB0aGUgbWFya2V0J3MgY3JlYXRvciDigJQgZm9yCmFnZW50LWNyZWF0ZWQgbWFya2V0cyB0aGF0IGlzIHRoZSB0cmVhc3VyeSBpdHNlbGYsIHZpYQpgYWdlbnRfY2xhaW1fbHBfZmVlc2AuAAAAE2NvbGxlY3RfbWFya2V0X2ZlZXMAAAAAAQAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAEAAAPpAAAACwAAAAM=",
        "AAAAAAAAAD9DbGFpbXMgdGhlIExQIHBvb2wgcmVtYWluZGVyIGZvciBhIG1hcmtldCB0aGUgdHJlYXN1cnkgY3JlYXRlZC4AAAAAFGNsYWltX3Bvb2xfcmVtYWluZGVyAAAAAQAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAEAAAPpAAAACwAAAAM=" ]),
      options
    )
  }
  public readonly fromJSON = {
    redeem: this.txFromJSON<Result<i128>>,
        deposit: this.txFromJSON<Result<void>>,
        withdraw: this.txFromJSON<Result<void>>,
        agent_buy: this.txFromJSON<Result<i128>>,
        agent_sell: this.txFromJSON<Result<i128>>,
        get_config: this.txFromJSON<Result<RiskConfig>>,
        set_paused: this.txFromJSON<Result<void>>,
        close_trade: this.txFromJSON<Result<i128>>,
        daily_usage: this.txFromJSON<Result<DailyUsage>>,
        get_net_cap: this.txFromJSON<Result<i128>>,
        agent_buy_v2: this.txFromJSON<Result<i128>>,
        net_exposure: this.txFromJSON<i128>,
        net_position: this.txFromJSON<NetPosition>,
        agent_sell_v2: this.txFromJSON<Result<i128>>,
        set_successor: this.txFromJSON<Result<void>>,
        daily_headroom: this.txFromJSON<Result<i128>>,
        migrate_market: this.txFromJSON<Result<void>>,
        market_valuation: this.txFromJSON<Result<Valuation>>,
        available_capital: this.txFromJSON<Result<i128>>,
        daily_usage_detail: this.txFromJSON<Result<UsageWindow>>,
        agent_claim_lp_fees: this.txFromJSON<Result<i128>>,
        agent_create_market: this.txFromJSON<Result<u64>>,
        collect_market_fees: this.txFromJSON<Result<i128>>,
        claim_pool_remainder: this.txFromJSON<Result<i128>>
  }
}