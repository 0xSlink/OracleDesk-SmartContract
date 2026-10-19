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




export const Errors = {
  1: {message:"NotInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"AgentAlreadyRegistered"},
  4: {message:"AgentNotRegistered"},
  5: {message:"TraceNotFound"},
  6: {message:"EmptyAction"},
  7: {message:"EmptyCid"},
  8: {message:"ChainHeadMismatch"},
  9: {message:"ChainedAgentMustChain"},
  10: {message:"TraceAlreadyConsumed"},
  11: {message:"AgentNotActive"},
  12: {message:"CannotReactivateRevoked"},
  13: {message:"FlagHistoryFull"},
  14: {message:"InvalidEffectiveTime"},
  15: {message:"UnknownMarket"},
  16: {message:"MarketRequired"},
  17: {message:"ProposalMustUseNoMarket"},
  18: {message:"UseCanonicalAction"},
  19: {message:"BatchNotFound"},
  20: {message:"InvalidBatch"},
  21: {message:"InvalidProof"},
  22: {message:"TooManyKeys"},
  23: {message:"KeyAlreadyBound"},
  24: {message:"InvalidKey"},
  25: {message:"InvalidSignature"},
  26: {message:"TraceNotSigned"}
}

export interface SigningKeyRecord {
  added_at: u64;
  public_key: Buffer;
  retired_at: Option<u64>;
}

export interface TraceBatch {
  agent: string;
  leaf_count: u32;
  manifest_cid: string;
  published_at: u64;
  root: Buffer;
}

export type TraceAction =
  | { tag: "CreateMarket", values: void }
  | { tag: "ProposeMarket", values: void }
  | { tag: "Buy", values: void }
  | { tag: "Sell", values: void }
  | { tag: "Skip", values: void }
  | { tag: "Redeem", values: void }
  | { tag: "ClaimRemainder", values: void }
  | { tag: "Resolve", values: void }
  | { tag: "Other", values: readonly [string] };

export type AgentStatus =
  | { tag: "Active", values: void }
  | { tag: "Retired", values: readonly [u64] }
  | { tag: "Revoked", values: readonly [u64, string] };

export interface AgentRecord {
  registered_at: u64;
  status: AgentStatus;
}

export interface TraceFlag {
  active: boolean;
  flagged_at: u64;
  reason: string;
}

export type TraceVerdict =
  | { tag: "Valid", values: void }
  | { tag: "HashMismatch", values: void }
  | { tag: "Flagged", values: readonly [string] }
  | { tag: "AgentRevokedAfter", values: readonly [u64] };

export interface Trace {
  action: TraceAction;
  agent: string;
  ipfs_cid: string;
  market_id: u64;
  published_at: u64;
  trace_hash: Buffer;
}

export type DataKey =
  | {tag: "Admin", values: void}
  | {tag: "NextId", values: void}
  | {tag: "Agent", values: readonly [string]}
  | {tag: "Trace", values: readonly [u64]}
  | {tag: "MarketTraceCount", values: readonly [u64]}
  | {tag: "MarketTrace", values: readonly [u64, u64]}
  | {tag: "Flag", values: readonly [u64]}
  | {tag: "FlagHistoryCount", values: readonly [u64]}
  | {tag: "FlagHistory", values: readonly [u64, u32]}
  | {tag: "MarketCore", values: void}
  | {tag: "Batch", values: readonly [u64]}
  | {tag: "NextBatchId", values: void}
  | {tag: "AgentBatchCount", values: readonly [string]}
  | {tag: "AgentBatch", values: readonly [string, u64]}
  | {tag: "AgentSigningKeys", values: readonly [string]}
  | {tag: "SigningKeyHistoryCount", values: readonly [string]}
  | {tag: "SigningKeyHistory", values: readonly [string, u32]}
  | {tag: "KeyToAgent", values: readonly [Buffer]}
  | {tag: "TraceSignature", values: readonly [u64]};

export interface Client {
  add_signing_key: ({agent, public_key, proof_sig}: {agent: string, public_key: Buffer, proof_sig: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  retire_signing_key: ({agent, public_key}: {agent: string, public_key: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  get_signing_keys: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<Array<Buffer>>>
  get_key_agent: ({public_key}: {public_key: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Option<string>>>
  signing_key_history_count: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>
  signing_key_history: ({agent, start, limit}: {agent: string, start: u32, limit: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<SigningKeyRecord>>>>
  publish_trace_signed: ({agent, market_id, action, trace_hash, ipfs_cid, signing_key, signature}: {agent: string, market_id: u64, action: TraceAction, trace_hash: Buffer, ipfs_cid: string, signing_key: Buffer, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>
  publish_chained_trace_signed: ({agent, market_id, action, trace_hash, ipfs_cid, expected_seq, prev_trace_hash, signing_key, signature}: {agent: string, market_id: u64, action: TraceAction, trace_hash: Buffer, ipfs_cid: string, expected_seq: u64, prev_trace_hash: Buffer, signing_key: Buffer, signature: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>
  get_trace_signature: ({trace_id}: {trace_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Option<readonly [Buffer, Buffer]>>>
  verify_trace_signature: ({trace_id}: {trace_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<boolean>>>
  get_trace: ({trace_id}: {trace_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Trace>>>
  trace_count: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>
  verify_trace: ({trace_id, received_hash}: {trace_id: u64, received_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<boolean>>>
  trace_verdict: ({trace_id, received_hash}: {trace_id: u64, received_hash: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<TraceVerdict>>>
  publish_trace_batch: ({agent, root, leaf_count, manifest_cid}: {agent: string, root: Buffer, leaf_count: u32, manifest_cid: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>
  get_batch: ({batch_id}: {batch_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<TraceBatch>>>
  batch_count: (options?: MethodOptions) => Promise<AssembledTransaction<u64>>
  agent_batch_count: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>
  agent_batch_at: ({agent, index}: {agent: string, index: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>
  verify_trace_inclusion: ({batch_id, trace_hash, index, proof}: {batch_id: u64, trace_hash: Buffer, index: u32, proof: Array<Buffer>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<boolean>>>
  compute_merkle_root: ({leaf_count, trace_hash, index, proof}: {leaf_count: u32, trace_hash: Buffer, index: u32, proof: Array<Buffer>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Buffer>>>
  is_registered: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>
  get_agent: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<Option<AgentRecord>>>
  publish_trace: ({agent, market_id, action, trace_hash, ipfs_cid}: {agent: string, market_id: u64, action: TraceAction, trace_hash: Buffer, ipfs_cid: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>
  register_agent: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  retire_agent: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  retire_self: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  reactivate_agent: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  revoke_agent: ({agent, effective_from, reason}: {agent: string, effective_from: u64, reason: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  flag_trace: ({trace_id, reason}: {trace_id: u64, reason: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  unflag_trace: ({trace_id, reason}: {trace_id: u64, reason: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  trace_flags: ({trace_id, start, limit}: {trace_id: u64, start: u32, limit: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<TraceFlag>>>>
  get_trace_flag: ({trace_id}: {trace_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Option<TraceFlag>>>
  market_trace_at: ({market_id, index}: {market_id: u64, index: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>
  unregister_agent: ({agent}: {agent: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>
  market_trace_count: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<u64>>
}
export class Client extends ContractClient {
  static async deploy<T = Client>(
        /** Constructor/Initialization Args for the contract's `__constructor` method */
        {admin, market_core}: {admin: string, market_core: string},
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
    return ContractClient.deploy({admin, market_core}, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABwAAAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADFVuYXV0aG9yaXplZAAAAAIAAAAAAAAAFkFnZW50QWxyZWFkeVJlZ2lzdGVyZWQAAAAAAAMAAAAAAAAAEkFnZW50Tm90UmVnaXN0ZXJlZAAAAAAABAAAAAAAAAANVHJhY2VOb3RGb3VuZAAAAAAAAAUAAAAAAAAAC0VtcHR5QWN0aW9uAAAAAAYAAAAAAAAACEVtcHR5Q2lkAAAABw==",
        "AAAAAQAAAAAAAAAAAAAABVRyYWNlAAAAAAAABgAAAAAAAAAGYWN0aW9uAAAAAAAQAAAAAAAAAAVhZ2VudAAAAAAAABMAAAAAAAAACGlwZnNfY2lkAAAAEAAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAAAAAAMcHVibGlzaGVkX2F0AAAABgAAAAAAAAAKdHJhY2VfaGFzaAAAAAAD7gAAACA=",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAABgAAAAAAAAAAAAAABUFkbWluAAAAAAAAAAAAAAAAAAAGTmV4dElkAAAAAAABAAAAAAAAAAVBZ2VudAAAAAAAAAEAAAATAAAAAQAAAAAAAAAFVHJhY2UAAAAAAAABAAAABgAAAAEAAAC+TnVtYmVyIG9mIHRyYWNlcyBwdWJsaXNoZWQgZm9yIGEgbWFya2V0LCBzbyBhIGNsaWVudCBjYW4gcGFnZSB0aHJvdWdoCmBNYXJrZXRUcmFjZShtYXJrZXRfaWQsIDAuLmNvdW50KWAgd2l0aG91dCB0aGUgY29udHJhY3QgZXZlciBob2xkaW5nCmFuIHVuYm91bmRlZCBWZWMgb2YgdHJhY2UgaWRzIGluIG9uZSBzdG9yYWdlIGVudHJ5LgAAAAAAEE1hcmtldFRyYWNlQ291bnQAAAABAAAABgAAAAEAAAAAAAAAC01hcmtldFRyYWNlAAAAAAIAAAAGAAAABg==",
        "AAAABQAAAAAAAAAAAAAADlRyYWNlUHVibGlzaGVkAAAAAAABAAAAD3RyYWNlX3B1Ymxpc2hlZAAAAAAHAAAAAAAAAAh0cmFjZV9pZAAAAAYAAAABAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAFYWdlbnQAAAAAAAATAAAAAQAAAAAAAAAGYWN0aW9uAAAAAAAQAAAAAAAAAAAAAAAKdHJhY2VfaGFzaAAAAAAD7gAAACAAAAAAAAAAAAAAAAhpcGZzX2NpZAAAABAAAAAAAAAAAAAAAAxwdWJsaXNoZWRfYXQAAAAGAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAD0FnZW50UmVnaXN0ZXJlZAAAAAABAAAAEGFnZW50X3JlZ2lzdGVyZWQAAAABAAAAAAAAAAVhZ2VudAAAAAAAABMAAAABAAAAAg==",
        "AAAAAAAAAAAAAAAJZ2V0X3RyYWNlAAAAAAAAAQAAAAAAAAAIdHJhY2VfaWQAAAAGAAAAAQAAA+kAAAfQAAAABVRyYWNlAAAAAAAAAw==",
        "AAAAAAAAAAAAAAALdHJhY2VfY291bnQAAAAAAAAAAAEAAAAG",
        "AAAAAAAAAAAAAAAMdmVyaWZ5X3RyYWNlAAAAAgAAAAAAAAAIdHJhY2VfaWQAAAAGAAAAAAAAAA1yZWNlaXZlZF9oYXNoAAAAAAAD7gAAACAAAAABAAAD6QAAAAEAAAAD",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAA=",
        "AAAAAAAAAAAAAAANaXNfcmVnaXN0ZXJlZAAAAAAAAAEAAAAAAAAABWFnZW50AAAAAAAAEwAAAAEAAAAB",
        "AAAAAAAAAAAAAAANcHVibGlzaF90cmFjZQAAAAAAAAUAAAAAAAAABWFnZW50AAAAAAAAEwAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAAAAAAGYWN0aW9uAAAAAAAQAAAAAAAAAAp0cmFjZV9oYXNoAAAAAAPuAAAAIAAAAAAAAAAIaXBmc19jaWQAAAAQAAAAAQAAA+kAAAAGAAAAAw==",
        "AAAAAAAAAAAAAAAOcmVnaXN0ZXJfYWdlbnQAAAAAAAEAAAAAAAAABWFnZW50AAAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAEFUaGUgYGluZGV4YC10aCAoMC1iYXNlZCwgcHVibGlzaCBvcmRlcikgdHJhY2UgaWQgZm9yIGBtYXJrZXRfaWRgLgAAAAAAAA9tYXJrZXRfdHJhY2VfYXQAAAAAAgAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAAAAAAFaW5kZXgAAAAAAAAGAAAAAQAAA+kAAAAGAAAAAw==",
        "AAAAAAAAAAAAAAAQdW5yZWdpc3Rlcl9hZ2VudAAAAAEAAAAAAAAABWFnZW50AAAAAAAAEwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAACtOdW1iZXIgb2YgdHJhY2VzIHB1Ymxpc2hlZCBmb3IgYG1hcmtldF9pZGAuAAAAABJtYXJrZXRfdHJhY2VfY291bnQAAAAAAAEAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAABg==" ]),
      options
    )
  }
  public readonly fromJSON = {
    get_trace: this.txFromJSON<Result<Trace>>,
    trace_count: this.txFromJSON<u64>,
    verify_trace: this.txFromJSON<Result<boolean>>,
    trace_verdict: this.txFromJSON<Result<TraceVerdict>>,
    publish_trace_batch: this.txFromJSON<Result<u64>>,
    get_batch: this.txFromJSON<Result<TraceBatch>>,
    batch_count: this.txFromJSON<u64>,
    agent_batch_count: this.txFromJSON<u64>,
    agent_batch_at: this.txFromJSON<Result<u64>>,
    verify_trace_inclusion: this.txFromJSON<Result<boolean>>,
    compute_merkle_root: this.txFromJSON<Result<Buffer>>,
    add_signing_key: this.txFromJSON<Result<void>>,
    retire_signing_key: this.txFromJSON<Result<void>>,
    get_signing_keys: this.txFromJSON<Array<Buffer>>,
    get_key_agent: this.txFromJSON<Option<string>>,
    signing_key_history_count: this.txFromJSON<u32>,
    signing_key_history: this.txFromJSON<Result<Array<SigningKeyRecord>>>,
    publish_trace_signed: this.txFromJSON<Result<u64>>,
    publish_chained_trace_signed: this.txFromJSON<Result<u64>>,
    get_trace_signature: this.txFromJSON<Option<readonly [Buffer, Buffer]>>,
    verify_trace_signature: this.txFromJSON<Result<boolean>>,
    is_registered: this.txFromJSON<boolean>,
    get_agent: this.txFromJSON<Option<AgentRecord>>,
    publish_trace: this.txFromJSON<Result<u64>>,
    register_agent: this.txFromJSON<Result<void>>,
    retire_agent: this.txFromJSON<Result<void>>,
    retire_self: this.txFromJSON<Result<void>>,
    reactivate_agent: this.txFromJSON<Result<void>>,
    revoke_agent: this.txFromJSON<Result<void>>,
    flag_trace: this.txFromJSON<Result<void>>,
    unflag_trace: this.txFromJSON<Result<void>>,
    trace_flags: this.txFromJSON<Result<Array<TraceFlag>>>,
    get_trace_flag: this.txFromJSON<Option<TraceFlag>>,
    market_trace_at: this.txFromJSON<Result<u64>>,
    unregister_agent: this.txFromJSON<Result<void>>,
    market_trace_count: this.txFromJSON<u64>
  }
}