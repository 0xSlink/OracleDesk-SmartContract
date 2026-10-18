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
  3: {message:"InvalidConfig"},
  4: {message:"MarketAlreadyConfigured"},
  5: {message:"MarketNotConfigured"},
  6: {message:"WrongMode"},
  7: {message:"MarketNotClosed"},
  8: {message:"PriceUnavailable"},
  9: {message:"StalePrice"},
  10: {message:"InvalidPrice"},
  11: {message:"NotSigner"},
  12: {message:"AlreadyAttested"},
  13: {message:"ThresholdNotReached"},
  14: {message:"DisputeActive"},
  15: {message:"DisputeWindowOpen"},
  16: {message:"NoProposal"},
  17: {message:"AlreadyFinalized"},
  18: {message:"SpecHashMismatch"},
  19: {message:"DeadlineNotReached"},
  20: {message:"ResolutionAvailable"},
  21: {message:"DomainMismatch"},
  22: {message:"StaleRound"},
  23: {message:"TooManySignatures"},
  24: {message:"DisputeWindowClosed"},
  25: {message:"NoDispute"},
  26: {message:"CommitteeNotFound"},
  27: {message:"RotationPending"},
  28: {message:"RotationNotReady"},
  29: {message:"RotationBoundExceeded"},
  30: {message:"DuplicateMember"},
  31: {message:"CommitteeTooLarge"},
  32: {message:"EvidenceDisabled"},
  33: {message:"EvidenceRequired"},
  34: {message:"EvidenceNotFound"},
  35: {message:"EvidenceAuthorMismatch"},
  36: {message:"EvidenceMarketMismatch"},
  37: {message:"EvidenceTooEarly"},
  38: {message:"EvidenceAlreadyUsed"},
  39: {message:"GuardianNotAllowed"},
  40: {message:"TooManyCancels"}
}

/**
 * The escalation authority's ruling. `Invalid` voids the market.
 */
export type Verdict = {tag: "Yes", values: void} | {tag: "No", values: void} | {tag: "Invalid", values: void};




/**
 * Reusable committee managed by an admin under time-locked rotation rules.
 */
export interface Committee {
  admin: string;
  epoch: u32;
  members: Array<string>;
  rotation_delay: u64;
  threshold: u32;
}

/**
 * Who rules on a dispute.
 */
export type Escalation = {tag: "Guardian", values: void} | {tag: "Committee", values: readonly [Array<string>, u32]};

/**
 * Which resolution path a market was on when it expired.
 */
export type ExpiryMode = {tag: "Unrevealed", values: void} | {tag: "Price", values: void} | {tag: "Signers", values: void};

export type OracleAsset = {tag: "Stellar", values: readonly [string]} | {tag: "Other", values: readonly [string]};


export interface PriceConfig {
  asset: OracleAsset;
  direction: PriceDirection;
  max_staleness: u64;
  reflector: string;
  threshold: i128;
}


export interface WeightedKey {
  /**
 * Raw Ed25519 public key. For a `G...` account address this is
 * `StrKey.decodeEd25519PublicKey(address)`; `C...` addresses have none.
 */
pubkey: Buffer;
  weight: u32;
}


/**
 * Binds a market to a reusable committee registry subject to rotation bounds.
 */
export interface CommitteeRef {
  committee_id: u64;
  dispute_window: u64;
  max_rotation_delta: u32;
  min_epoch: u32;
}


/**
 * Dual-path resolution mode: price feed first, falling back to signers.
 */
export interface HybridConfig {
  fallback: SignerConfig;
  fallback_after: u64;
  price: PriceConfig;
}


export interface SignerConfig {
  dispute_window: u64;
  signers: Array<string>;
  threshold: u32;
}


/**
 * Evidence policy governing whether attestation evidence is required.
 */
export type EvidencePolicy = {tag: "Disabled", values: void} | {tag: "Optional", values: void} | {tag: "Required", values: void};

/**
 * Reason code constraining guardian interventions.
 */
export type GuardianReason = {tag: "UnrevealedSpec", values: void} | {tag: "OracleUnavailable", values: void} | {tag: "SignersStalled", values: void} | {tag: "DisputeUpheld", values: void};

export type PriceDirection = {tag: "Above", values: void} | {tag: "AtOrAbove", values: void} | {tag: "Below", values: void} | {tag: "AtOrBelow", values: void};

/**
 * The full resolution rule a market's `resolution_hash` commits to. Hashing
 * this (rather than the bare `PriceConfig`/`SignerConfig`) keeps the two
 * modes in disjoint hash spaces even if their field encodings could
 * otherwise collide. See `docs/adr/0001-resolution-commitment.md`.
 */
export type ResolutionSpec = {tag: "Price", values: readonly [PriceConfig]} | {tag: "Signers", values: readonly [SignerConfig]} | {tag: "WeightedKeys", values: readonly [WeightedKeyConfig]} | {tag: "SignersDisputable", values: readonly [DisputableSignerConfig]} | {tag: "Committee", values: readonly [CommitteeRef]} | {tag: "Hybrid", values: readonly [HybridConfig]};




export type ResolutionState = {tag: "Unconfigured", values: void} | {tag: "SignersPending", values: void} | {tag: "Finalized", values: void} | {tag: "Expired", values: void} | {tag: "Disputed", values: void} | {tag: "HybridPricePending", values: void} | {tag: "HybridFallbackOpen", values: void};



/**
 * Versioned commitment envelope. Unlike a bare [`ResolutionSpec`] it binds the
 * network and the market-core instance, and carries a creator-chosen `salt`
 * that makes a pre-reveal brute-force of the (low-entropy) rule infeasible.
 */
export interface ResolutionSpecV2 {
  market_core: string;
  /**
 * `sha256(network passphrase)`, i.e. `env.ledger().network_id()`.
 */
network_id: Buffer;
  rule: ResolutionSpec;
  salt: Buffer;
  version: u32;
}





export interface WeightedKeyConfig {
  dispute_window: u64;
  signers: Array<WeightedKey>;
  /**
 * Total weight that must sign one outcome. Choose it above half the total
 * weight so two outcomes can never both reach it.
 */
weight_threshold: u32;
}


/**
 * Guardian action type emitted for audit trails.
 */
export type GuardianActionKind = {tag: "Void", values: void} | {tag: "CancelProposal", values: void};




/**
 * Stored link connecting an attestation to a reasoning registry trace.
 */
export interface AttestationEvidence {
  attested_at: u64;
  outcome: Outcome;
  signer: string;
  trace_hash: Buffer;
  trace_id: u64;
}



/**
 * Attested resolution whose proposal can be challenged with a bond during
 * the dispute window. Hashed as `ResolutionSpec::SignersDisputable`, a
 * separate variant, so existing `Signers` commitments are unaffected.
 */
export interface DisputableSignerConfig {
  /**
 * Collateral escrowed by `dispute`; must be positive.
 */
dispute_bond: i128;
  dispute_window: u64;
  escalation: Escalation;
  /**
 * Seconds after a dispute opens before anyone may void it unresolved.
 */
escalation_timeout: u64;
  signers: Array<string>;
  threshold: u32;
}

export type Outcome = {tag: "Yes", values: void} | {tag: "No", values: void};

export interface Client {
  /**
   * Construct and simulate a state transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  state: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<ResolutionState>>

  /**
   * Construct and simulate a attest transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  attest: ({market_id, signer, outcome}: {market_id: u64, signer: string, outcome: Outcome}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a dispute transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Challenges the current proposal of a `SignersDisputable` market while
   * its dispute window is still open (strictly before `proposed_at +
   * dispute_window`, the instant `finalize_signers` becomes callable). The
   * disputer escrows `dispute_bond` of the collateral token, which freezes
   * finalization until the escalation authority rules or
   * `escalation_timeout` lapses. `alternative` is the outcome the disputer
   * claims is correct (`None`: the market is invalid); it only informs the
   * ruling and must differ from the proposal. The disputer may also be a
   * proposing signer.
   */
  dispute: ({market_id, disputer, alternative}: {market_id: u64, disputer: string, alternative: Option<Outcome>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a evidence_at transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reads the evidence citation at `index` for `market_id`.
   */
  evidence_at: ({market_id, index}: {market_id: u64, index: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<AttestationEvidence>>>

  /**
   * Construct and simulate a cancel_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Total proposal cancellations executed by the guardian for `market_id`.
   */
  cancel_count: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a current_round transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Current attestation round of a weighted-key market; the `round`
   * argument of `submit_signatures` and part of every signed message.
   */
  current_round: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a expire_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Permissionless liveness fallback: once `resolution_deadline` has passed,
   * anyone may void a market that is still unresolved, so collateral is
   * never locked behind a missing reveal, a dead feed or silent signers.
   * 
   * Resolution is preferred over voiding. A price market whose feed is
   * valid right now is *resolved* instead (so a stalled caller can never
   * force a void on a resolvable market), and a market with a pending
   * signer proposal is refused with `ResolutionAvailable` until
   * `finalize_signers` settles it.
   */
  expire_market: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_committee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reads committee state and extends its storage TTL.
   */
  get_committee: ({committee_id}: {committee_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Committee>>>

  /**
   * Construct and simulate a open_fallback transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Opens fallback signer attestation in a hybrid market if the feed failed past the deadline.
   */
  open_fallback: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a resolve_price transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  resolve_price: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Outcome>>>

  /**
   * Construct and simulate a evidence_count transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Total evidence citations recorded for `market_id`.
   */
  evidence_count: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a expire_dispute transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Permissionless fallback when the escalation authority never rules:
   * `escalation_timeout` after the dispute opened, the bond is refunded and
   * the market is voided, so neither side wins by default.
   */
  expire_dispute: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a settle_dispute transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Guardian escalation: the admin rules on an open dispute.
   */
  settle_dispute: ({market_id, verdict}: {market_id: u64, verdict: Verdict}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Guardian action: withdraws an unfinalized signer proposal during the dispute window
   * subject to a rate limit of MAX_CANCELS_PER_MARKET.
   */
  cancel_proposal: ({market_id, reason}: {market_id: u64, reason: GuardianReason}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_rotation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Cancels a pending rotation for a committee (admin only).
   */
  cancel_rotation: ({committee_id}: {committee_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a evidence_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reads the currently configured evidence settings, if any.
   */
  evidence_config: (options?: MethodOptions) => Promise<AssembledTransaction<Option<readonly [string, EvidencePolicy]>>>

  /**
   * Construct and simulate a create_committee transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Creates a reusable committee with an admin, member set, threshold, and time-lock delay.
   */
  create_committee: ({admin, members, threshold, rotation_delay}: {admin: string, members: Array<string>, threshold: u32, rotation_delay: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a execute_rotation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Permissionlessly executes a matured rotation for a committee, bumping its epoch.
   */
  execute_rotation: ({committee_id}: {committee_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a finalize_signers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  finalize_signers: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Outcome>>>

  /**
   * Construct and simulate a guardian_options transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the guardian reason codes valid for `market_id` right now.
   */
  guardian_options: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Array<GuardianReason>>>

  /**
   * Construct and simulate a propose_rotation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Proposes a new member set and threshold for a committee, executable after rotation_delay.
   */
  propose_rotation: ({committee_id, members, threshold}: {committee_id: u64, members: Array<string>, threshold: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a register_spec_v2 transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reveals a rule committed to through a [`ResolutionSpecV2`] envelope.
   * The envelope must name this network and this resolver's market-core
   * (`DomainMismatch`), use a non-zero salt and the current `version`
   * (`InvalidConfig`), and hash to the market's commitment
   * (`SpecHashMismatch`). The inner rule is then validated and stored
   * exactly as the legacy entrypoints do.
   */
  register_spec_v2: ({market_id, spec}: {market_id: u64, spec: ResolutionSpecV2}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a escalation_attest transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Committee escalation: each member votes once; the first verdict to
   * gather `k` votes settles the dispute.
   */
  escalation_attest: ({market_id, member, verdict}: {market_id: u64, member: string, verdict: Verdict}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a submit_signatures transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Counts Ed25519 signatures from a weighted committee. Permissionless: the
   * signatures, not the submitter, carry the authority. Each signature is
   * over the message built by `attestation_message`; non-members and keys
   * that already signed this round (for either outcome) are skipped, and a
   * single invalid signature aborts the whole batch, so submitters should
   * pre-filter with a verifier. Once the accumulated weight for `outcome`
   * first reaches the threshold a proposal is recorded and the dispute
   * window starts; later calls never move it.
   */
  submit_signatures: ({market_id, outcome, round, sigs}: {market_id: u64, outcome: Outcome, round: u32, sigs: Array<readonly [Buffer, Buffer]>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a claim_dispute_bond transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Pays a settled dispute's bond to its payee (the disputer, or the
   * market-core treasury when the proposal was upheld). Permissionless.
   */
  claim_dispute_bond: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_price_spec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Reveals the resolution rule committed to at market creation. Anyone
   * may call this (there is no admin gate) because the only thing that
   * makes a spec acceptable is that it hashes to `market.resolution_hash`
   * — the commitment, not the caller, is the authority. Fails if a spec
   * was already registered for this market.
   */
  register_price_spec: ({market_id, spec}: {market_id: u64, spec: PriceConfig}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a resolution_deadline transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * When `expire_market` becomes callable for `market_id`: `close_time` plus
   * [`RESOLUTION_GRACE_SECS`]. Needs only market-core's `close_time`, so it
   * is known before (and without) a spec reveal.
   */
  resolution_deadline: ({market_id}: {market_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a set_evidence_config transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Configures the reasoning registry contract address and evidence policy (admin only).
   */
  set_evidence_config: ({registry, policy}: {registry: string, policy: EvidencePolicy}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a attest_with_evidence transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Attests to an outcome backed by an on-chain reasoning trace in reasoning-registry.
   */
  attest_with_evidence: ({market_id, signer, outcome, trace_id}: {market_id: u64, signer: string, outcome: Outcome, trace_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a guardian_void_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Guardian action: voids a market subject to verified on-chain reason codes and grace periods.
   */
  guardian_void_market: ({market_id, reason}: {market_id: u64, reason: GuardianReason}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_hybrid_spec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Registers a hybrid resolution specification (price feed with attested fallback).
   */
  register_hybrid_spec: ({market_id, spec}: {market_id: u64, spec: HybridConfig}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_signer_spec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * See `register_price_spec` — same commit/reveal rule, signer mode.
   */
  register_signer_spec: ({market_id, spec}: {market_id: u64, spec: SignerConfig}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_committee_spec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Registers a reusable committee specification for `market_id`.
   */
  register_committee_spec: ({market_id, spec}: {market_id: u64, spec: CommitteeRef}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_disputable_spec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * See `register_price_spec` — same commit/reveal rule, signer mode whose
   * proposals can be challenged with a bond (see `dispute`).
   */
  register_disputable_spec: ({market_id, spec}: {market_id: u64, spec: DisputableSignerConfig}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a register_weighted_key_spec transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * See `register_price_spec` — same commit/reveal rule, weighted Ed25519
   * committee that signs off-chain and is verified by `submit_signatures`.
   */
  register_weighted_key_spec: ({market_id, spec}: {market_id: u64, spec: WeightedKeyConfig}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAAKAAAAAAAAAAOTm90SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADFVuYXV0aG9yaXplZAAAAAIAAAAAAAAADUludmFsaWRDb25maWcAAAAAAAADAAAAAAAAABdNYXJrZXRBbHJlYWR5Q29uZmlndXJlZAAAAAAEAAAAAAAAABNNYXJrZXROb3RDb25maWd1cmVkAAAAAAUAAAAAAAAACVdyb25nTW9kZQAAAAAAAAYAAAAAAAAAD01hcmtldE5vdENsb3NlZAAAAAAHAAAAAAAAABBQcmljZVVuYXZhaWxhYmxlAAAACAAAAAAAAAAKU3RhbGVQcmljZQAAAAAACQAAAAAAAAAMSW52YWxpZFByaWNlAAAACgAAAAAAAAAJTm90U2lnbmVyAAAAAAAACwAAAAAAAAAPQWxyZWFkeUF0dGVzdGVkAAAAAAwAAAAAAAAAE1RocmVzaG9sZE5vdFJlYWNoZWQAAAAADQAAAAAAAAANRGlzcHV0ZUFjdGl2ZQAAAAAAAA4AAAAAAAAAEURpc3B1dGVXaW5kb3dPcGVuAAAAAAAADwAAAAAAAAAKTm9Qcm9wb3NhbAAAAAAAEAAAAAAAAAAQQWxyZWFkeUZpbmFsaXplZAAAABEAAAAAAAAAEFNwZWNIYXNoTWlzbWF0Y2gAAAASAAAAAAAAABJEZWFkbGluZU5vdFJlYWNoZWQAAAAAABMAAAAAAAAAE1Jlc29sdXRpb25BdmFpbGFibGUAAAAAFAAAAAAAAAAORG9tYWluTWlzbWF0Y2gAAAAAABUAAAAAAAAAClN0YWxlUm91bmQAAAAAABYAAAAAAAAAEVRvb01hbnlTaWduYXR1cmVzAAAAAAAAFwAAAAAAAAATRGlzcHV0ZVdpbmRvd0Nsb3NlZAAAAAAYAAAAAAAAAAlOb0Rpc3B1dGUAAAAAAAAZAAAAAAAAABFDb21taXR0ZWVOb3RGb3VuZAAAAAAAABoAAAAAAAAAD1JvdGF0aW9uUGVuZGluZwAAAAAbAAAAAAAAABBSb3RhdGlvbk5vdFJlYWR5AAAAHAAAAAAAAAAVUm90YXRpb25Cb3VuZEV4Y2VlZGVkAAAAAAAAHQAAAAAAAAAPRHVwbGljYXRlTWVtYmVyAAAAAB4AAAAAAAAAEUNvbW1pdHRlZVRvb0xhcmdlAAAAAAAAHwAAAAAAAAAQRXZpZGVuY2VEaXNhYmxlZAAAACAAAAAAAAAAEEV2aWRlbmNlUmVxdWlyZWQAAAAhAAAAAAAAABBFdmlkZW5jZU5vdEZvdW5kAAAAIgAAAAAAAAAWRXZpZGVuY2VBdXRob3JNaXNtYXRjaAAAAAAAIwAAAAAAAAAWRXZpZGVuY2VNYXJrZXRNaXNtYXRjaAAAAAAAJAAAAAAAAAAQRXZpZGVuY2VUb29FYXJseQAAACUAAAAAAAAAE0V2aWRlbmNlQWxyZWFkeVVzZWQAAAAAJgAAAAAAAAASR3VhcmRpYW5Ob3RBbGxvd2VkAAAAAAAnAAAAAAAAAA5Ub29NYW55Q2FuY2VscwAAAAAAKA==",
        "AAAAAgAAAD5UaGUgZXNjYWxhdGlvbiBhdXRob3JpdHkncyBydWxpbmcuIGBJbnZhbGlkYCB2b2lkcyB0aGUgbWFya2V0LgAAAAAAAAAAAAdWZXJkaWN0AAAAAAMAAAAAAAAAAAAAAANZZXMAAAAAAAAAAAAAAAACTm8AAAAAAAAAAAAAAAAAB0ludmFsaWQA",
        "AAAABQAAAAAAAAAAAAAACEF0dGVzdGVkAAAAAQAAAAhhdHRlc3RlZAAAAAUAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAAAAAAAAZzaWduZXIAAAAAABMAAAABAAAAAAAAAAdvdXRjb21lAAAAB9AAAAAHT3V0Y29tZQAAAAAAAAAAAAAAAAh0cmFjZV9pZAAAA+gAAAAGAAAAAAAAAAAAAAAFY291bnQAAAAAAAAEAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAACERpc3B1dGVkAAAAAQAAAAhkaXNwdXRlZAAAAAQAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAAAAAAAAhkaXNwdXRlcgAAABMAAAAAAAAAAAAAAARib25kAAAACwAAAAAAAABJV2hhdCB0aGUgZGlzcHV0ZXIgY2xhaW1zIGlzIGNvcnJlY3Q7IGBOb25lYCBjbGFpbXMgdGhlIG1hcmtldCBpcyBpbnZhbGlkLgAAAAAAAAthbHRlcm5hdGl2ZQAAAAPoAAAH0AAAAAdPdXRjb21lAAAAAAAAAAAC",
        "AAAAAQAAAEhSZXVzYWJsZSBjb21taXR0ZWUgbWFuYWdlZCBieSBhbiBhZG1pbiB1bmRlciB0aW1lLWxvY2tlZCByb3RhdGlvbiBydWxlcy4AAAAAAAAACUNvbW1pdHRlZQAAAAAAAAUAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAFZXBvY2gAAAAAAAAEAAAAAAAAAAdtZW1iZXJzAAAAA+oAAAATAAAAAAAAAA5yb3RhdGlvbl9kZWxheQAAAAAABgAAAAAAAAAJdGhyZXNob2xkAAAAAAAABA==",
        "AAAAAgAAABdXaG8gcnVsZXMgb24gYSBkaXNwdXRlLgAAAAAAAAAACkVzY2FsYXRpb24AAAAAAAIAAAAAAAAALVRoZSByZXNvbHZlciBhZG1pbiwgdGhyb3VnaCBgc2V0dGxlX2Rpc3B1dGVgLgAAAAAAAAhHdWFyZGlhbgAAAAEAAABfYGtgIG9mIHRoZXNlIG1lbWJlcnMsIHRocm91Z2ggYGVzY2FsYXRpb25fYXR0ZXN0YC4gTXVzdCBiZSBkaXNqb2ludApmcm9tIHRoZSBwcm9wb3Npbmcgc2lnbmVycy4AAAAACUNvbW1pdHRlZQAAAAAAAAIAAAPqAAAAEwAAAAQ=",
        "AAAAAgAAADZXaGljaCByZXNvbHV0aW9uIHBhdGggYSBtYXJrZXQgd2FzIG9uIHdoZW4gaXQgZXhwaXJlZC4AAAAAAAAAAAAKRXhwaXJ5TW9kZQAAAAAAAwAAAAAAAAAAAAAAClVucmV2ZWFsZWQAAAAAAAAAAAAAAAAABVByaWNlAAAAAAAAAAAAAAAAAAAHU2lnbmVycwA=",
        "AAAAAgAAAAAAAAAAAAAAC09yYWNsZUFzc2V0AAAAAAIAAAABAAAAAAAAAAdTdGVsbGFyAAAAAAEAAAATAAAAAQAAAAAAAAAFT3RoZXIAAAAAAAABAAAAEQ==",
        "AAAAAQAAAAAAAAAAAAAAC1ByaWNlQ29uZmlnAAAAAAUAAAAAAAAABWFzc2V0AAAAAAAH0AAAAAtPcmFjbGVBc3NldAAAAAAAAAAACWRpcmVjdGlvbgAAAAAAB9AAAAAOUHJpY2VEaXJlY3Rpb24AAAAAAAAAAAANbWF4X3N0YWxlbmVzcwAAAAAAAAYAAAAAAAAACXJlZmxlY3RvcgAAAAAAABMAAAAAAAAACXRocmVzaG9sZAAAAAAAAAs=",
        "AAAAAQAAAAAAAAAAAAAAC1dlaWdodGVkS2V5AAAAAAIAAACCUmF3IEVkMjU1MTkgcHVibGljIGtleS4gRm9yIGEgYEcuLi5gIGFjY291bnQgYWRkcmVzcyB0aGlzIGlzCmBTdHJLZXkuZGVjb2RlRWQyNTUxOVB1YmxpY0tleShhZGRyZXNzKWA7IGBDLi4uYCBhZGRyZXNzZXMgaGF2ZSBub25lLgAAAAAABnB1YmtleQAAAAAD7gAAACAAAAAAAAAABndlaWdodAAAAAAABA==",
        "AAAAAQAAAEtCaW5kcyBhIG1hcmtldCB0byBhIHJldXNhYmxlIGNvbW1pdHRlZSByZWdpc3RyeSBzdWJqZWN0IHRvIHJvdGF0aW9uIGJvdW5kcy4AAAAAAAAAAAxDb21taXR0ZWVSZWYAAAAEAAAAAAAAAAxjb21taXR0ZWVfaWQAAAAGAAAAAAAAAA5kaXNwdXRlX3dpbmRvdwAAAAAABgAAAAAAAAASbWF4X3JvdGF0aW9uX2RlbHRhAAAAAAAEAAAAAAAAAAltaW5fZXBvY2gAAAAAAAAE",
        "AAAAAQAAAEVEdWFsLXBhdGggcmVzb2x1dGlvbiBtb2RlOiBwcmljZSBmZWVkIGZpcnN0LCBmYWxsaW5nIGJhY2sgdG8gc2lnbmVycy4AAAAAAAAAAAAADEh5YnJpZENvbmZpZwAAAAMAAAAAAAAACGZhbGxiYWNrAAAH0AAAAAxTaWduZXJDb25maWcAAAAAAAAADmZhbGxiYWNrX2FmdGVyAAAAAAAGAAAAAAAAAAVwcmljZQAAAAAAB9AAAAALUHJpY2VDb25maWcA",
        "AAAAAQAAAAAAAAAAAAAADFNpZ25lckNvbmZpZwAAAAMAAAAAAAAADmRpc3B1dGVfd2luZG93AAAAAAAGAAAAAAAAAAdzaWduZXJzAAAAA+oAAAATAAAAAAAAAAl0aHJlc2hvbGQAAAAAAAAE",
        "AAAABQAAAAAAAAAAAAAADU1hcmtldEV4cGlyZWQAAAAAAAABAAAADm1hcmtldF9leHBpcmVkAAAAAAAEAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAKY2xvc2VfdGltZQAAAAAABgAAAAAAAAAAAAAACGRlYWRsaW5lAAAABgAAAAAAAAAAAAAABG1vZGUAAAfQAAAACkV4cGlyeU1vZGUAAAAAAAAAAAAC",
        "AAAAAgAAAENFdmlkZW5jZSBwb2xpY3kgZ292ZXJuaW5nIHdoZXRoZXIgYXR0ZXN0YXRpb24gZXZpZGVuY2UgaXMgcmVxdWlyZWQuAAAAAAAAAAAORXZpZGVuY2VQb2xpY3kAAAAAAAMAAAAAAAAAAAAAAAhEaXNhYmxlZAAAAAAAAAAAAAAACE9wdGlvbmFsAAAAAAAAAAAAAAAIUmVxdWlyZWQ=",
        "AAAAAgAAADBSZWFzb24gY29kZSBjb25zdHJhaW5pbmcgZ3VhcmRpYW4gaW50ZXJ2ZW50aW9ucy4AAAAAAAAADkd1YXJkaWFuUmVhc29uAAAAAAAEAAAAAAAAADZQYXN0IGNsb3NlX3RpbWUgKyBSRVZFQUxfR1JBQ0VfU0VDUywgbm8gc3BlYyByZXZlYWxlZC4AAAAAAA5VbnJldmVhbGVkU3BlYwAAAAAAAAAAAFFQcmljZSBtb2RlLCBwYXN0IGNsb3NlX3RpbWUgKyBPUkFDTEVfR1JBQ0VfU0VDUywgcHJpY2UgZmVlZCB1bmF2YWlsYWJsZSBvciBzdGFsZS4AAAAAAAART3JhY2xlVW5hdmFpbGFibGUAAAAAAAAAAAAAPlNpZ25lciBtb2RlLCBwYXN0IGNsb3NlX3RpbWUgKyBTSUdORVJfR1JBQ0VfU0VDUywgbm8gcHJvcG9zYWwuAAAAAAAOU2lnbmVyc1N0YWxsZWQAAAAAAAAAAAA3UHJvcG9zYWwgZXhpc3RzIGFuZCBkaXNwdXRlIHdpbmRvdyBpcyBjdXJyZW50bHkgYWN0aXZlLgAAAAANRGlzcHV0ZVVwaGVsZAAAAA==",
        "AAAAAgAAAAAAAAAAAAAADlByaWNlRGlyZWN0aW9uAAAAAAAEAAAAAAAAAAAAAAAFQWJvdmUAAAAAAAAAAAAAAAAAAAlBdE9yQWJvdmUAAAAAAAAAAAAAAAAAAAVCZWxvdwAAAAAAAAAAAAAAAAAACUF0T3JCZWxvdwAAAA==",
        "AAAAAgAAARNUaGUgZnVsbCByZXNvbHV0aW9uIHJ1bGUgYSBtYXJrZXQncyBgcmVzb2x1dGlvbl9oYXNoYCBjb21taXRzIHRvLiBIYXNoaW5nCnRoaXMgKHJhdGhlciB0aGFuIHRoZSBiYXJlIGBQcmljZUNvbmZpZ2AvYFNpZ25lckNvbmZpZ2ApIGtlZXBzIHRoZSB0d28KbW9kZXMgaW4gZGlzam9pbnQgaGFzaCBzcGFjZXMgZXZlbiBpZiB0aGVpciBmaWVsZCBlbmNvZGluZ3MgY291bGQKb3RoZXJ3aXNlIGNvbGxpZGUuIFNlZSBgZG9jcy9hZHIvMDAwMS1yZXNvbHV0aW9uLWNvbW1pdG1lbnQubWRgLgAAAAAAAAAADlJlc29sdXRpb25TcGVjAAAAAAAGAAAAAQAAAAAAAAAFUHJpY2UAAAAAAAABAAAH0AAAAAtQcmljZUNvbmZpZwAAAAABAAAAAAAAAAdTaWduZXJzAAAAAAEAAAfQAAAADFNpZ25lckNvbmZpZwAAAAEAAAAAAAAADFdlaWdodGVkS2V5cwAAAAEAAAfQAAAAEVdlaWdodGVkS2V5Q29uZmlnAAAAAAAAAQAAAAAAAAARU2lnbmVyc0Rpc3B1dGFibGUAAAAAAAABAAAH0AAAABZEaXNwdXRhYmxlU2lnbmVyQ29uZmlnAAAAAAABAAAAAAAAAAlDb21taXR0ZWUAAAAAAAABAAAH0AAAAAxDb21taXR0ZWVSZWYAAAABAAAAAAAAAAZIeWJyaWQAAAAAAAEAAAfQAAAADEh5YnJpZENvbmZpZw==",
        "AAAABQAAAVlgZGlzcHV0ZXJfd29uYCBpcyB0cnVlIHdoZW4gdGhlIGJvbmQgcmV0dXJucyB0byB0aGUgZGlzcHV0ZXIgYmVjYXVzZSB0aGUKcnVsaW5nIG92ZXJ0dXJuZWQgdGhlIHByb3Bvc2FsLiBBIHRpbWVvdXQgKGBleHBpcmVfZGlzcHV0ZWApIGFsc28gcmVmdW5kcwp0aGUgYm9uZCBidXQgaXMgcmVwb3J0ZWQgYXMgYHZlcmRpY3Q6IEludmFsaWQsIGRpc3B1dGVyX3dvbjogZmFsc2VgOiBhCnJ1bGluZyBvZiBgSW52YWxpZGAgYWx3YXlzIG92ZXJ0dXJucywgc28gdGhhdCBwYWlyIGlkZW50aWZpZXMgYSB0aW1lb3V0LgpgcGF5b3V0YCBpcyB0aGUgZXNjcm93ZWQgYm9uZCwgY2xhaW1hYmxlIGJ5IGl0cyBwYXllZS4AAAAAAAAAAAAADkRpc3B1dGVTZXR0bGVkAAAAAAABAAAAD2Rpc3B1dGVfc2V0dGxlZAAAAAAEAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAHdmVyZGljdAAAAAfQAAAAB1ZlcmRpY3QAAAAAAAAAAAAAAAAMZGlzcHV0ZXJfd29uAAAAAQAAAAAAAAAAAAAABnBheW91dAAAAAAACwAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAADkZhbGxiYWNrT3BlbmVkAAAAAAABAAAAD2ZhbGxiYWNrX29wZW5lZAAAAAADAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAJb3BlbmVkX2F0AAAAAAAABgAAAAAAAAAAAAAACmxhc3RfZXJyb3IAAAAAAAQAAAAAAAAAAg==",
        "AAAABQAAAAAAAAAAAAAADkd1YXJkaWFuQWN0aW9uAAAAAAABAAAAD2d1YXJkaWFuX2FjdGlvbgAAAAAEAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAGYWN0aW9uAAAAAAfQAAAAEkd1YXJkaWFuQWN0aW9uS2luZAAAAAAAAAAAAAAAAAAGcmVhc29uAAAAAAfQAAAADkd1YXJkaWFuUmVhc29uAAAAAAAAAAAAAAAAAAJhdAAAAAAABgAAAAAAAAAC",
        "AAAAAgAAAAAAAAAAAAAAD1Jlc29sdXRpb25TdGF0ZQAAAAAHAAAAAAAAAAAAAAAMVW5jb25maWd1cmVkAAAAAAAAAAAAAAAOU2lnbmVyc1BlbmRpbmcAAAAAAAAAAAAAAAAACUZpbmFsaXplZAAAAAAAAAAAAABEVm9pZGVkIGJ5IFtgUmVzb2x2ZXI6OmV4cGlyZV9tYXJrZXRgXSBhZnRlciB0aGUgcmVzb2x1dGlvbiBkZWFkbGluZS4AAAAHRXhwaXJlZAAAAAAAAAAAQkEgYm9uZGVkIGRpc3B1dGUgaXMgb3BlbjsgZmluYWxpemF0aW9uIGlzIGZyb3plbiB1bnRpbCBpdCBzZXR0bGVzLgAAAAAACERpc3B1dGVkAAAAAAAAAAAAAAASSHlicmlkUHJpY2VQZW5kaW5nAAAAAAAAAAAAAAAAABJIeWJyaWRGYWxsYmFja09wZW4AAA==",
        "AAAABQAAAAAAAAAAAAAAD1ByaWNlUmVzb2x1dGlvbgAAAAABAAAAEHByaWNlX3Jlc29sdXRpb24AAAAEAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAAAAAAFcHJpY2UAAAAAAAALAAAAAAAAAAAAAAAPcHJpY2VfdGltZXN0YW1wAAAAAAYAAAAAAAAAAAAAAAdvdXRjb21lAAAAB9AAAAAHT3V0Y29tZQAAAAAAAAAAAg==",
        "AAAAAQAAAOBWZXJzaW9uZWQgY29tbWl0bWVudCBlbnZlbG9wZS4gVW5saWtlIGEgYmFyZSBbYFJlc29sdXRpb25TcGVjYF0gaXQgYmluZHMgdGhlCm5ldHdvcmsgYW5kIHRoZSBtYXJrZXQtY29yZSBpbnN0YW5jZSwgYW5kIGNhcnJpZXMgYSBjcmVhdG9yLWNob3NlbiBgc2FsdGAKdGhhdCBtYWtlcyBhIHByZS1yZXZlYWwgYnJ1dGUtZm9yY2Ugb2YgdGhlIChsb3ctZW50cm9weSkgcnVsZSBpbmZlYXNpYmxlLgAAAAAAAAAQUmVzb2x1dGlvblNwZWNWMgAAAAUAAAAAAAAAC21hcmtldF9jb3JlAAAAABMAAAA/YHNoYTI1NihuZXR3b3JrIHBhc3NwaHJhc2UpYCwgaS5lLiBgZW52LmxlZGdlcigpLm5ldHdvcmtfaWQoKWAuAAAAAApuZXR3b3JrX2lkAAAAAAPuAAAAIAAAAAAAAAAEcnVsZQAAB9AAAAAOUmVzb2x1dGlvblNwZWMAAAAAAAAAAAAEc2FsdAAAA+4AAAAgAAAAAAAAAAd2ZXJzaW9uAAAAAAQ=",
        "AAAABQAAAAAAAAAAAAAAEENvbW1pdHRlZUNyZWF0ZWQAAAABAAAAEWNvbW1pdHRlZV9jcmVhdGVkAAAAAAAABAAAAAAAAAAMY29tbWl0dGVlX2lkAAAABgAAAAEAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAAAAAAACXRocmVzaG9sZAAAAAAAAAQAAAAAAAAAAAAAAAdtZW1iZXJzAAAAA+oAAAATAAAAAAAAAAI=",
        "AAAABQAAAAAAAAAAAAAAEFJvdGF0aW9uRXhlY3V0ZWQAAAABAAAAEXJvdGF0aW9uX2V4ZWN1dGVkAAAAAAAAAgAAAAAAAAAMY29tbWl0dGVlX2lkAAAABgAAAAEAAAAAAAAABWVwb2NoAAAAAAAABAAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAEFJvdGF0aW9uUHJvcG9zZWQAAAABAAAAEXJvdGF0aW9uX3Byb3Bvc2VkAAAAAAAAAgAAAAAAAAAMY29tbWl0dGVlX2lkAAAABgAAAAEAAAAAAAAADWV4ZWN1dGFibGVfYXQAAAAAAAAGAAAAAAAAAAI=",
        "AAAAAQAAAAAAAAAAAAAAEVdlaWdodGVkS2V5Q29uZmlnAAAAAAAAAwAAAAAAAAAOZGlzcHV0ZV93aW5kb3cAAAAAAAYAAAAAAAAAB3NpZ25lcnMAAAAD6gAAB9AAAAALV2VpZ2h0ZWRLZXkAAAAAd1RvdGFsIHdlaWdodCB0aGF0IG11c3Qgc2lnbiBvbmUgb3V0Y29tZS4gQ2hvb3NlIGl0IGFib3ZlIGhhbGYgdGhlIHRvdGFsCndlaWdodCBzbyB0d28gb3V0Y29tZXMgY2FuIG5ldmVyIGJvdGggcmVhY2ggaXQuAAAAABB3ZWlnaHRfdGhyZXNob2xkAAAABA==",
        "AAAABQAAAAAAAAAAAAAAEVJvdGF0aW9uQ2FuY2VsbGVkAAAAAAAAAQAAABJyb3RhdGlvbl9jYW5jZWxsZWQAAAAAAAEAAAAAAAAADGNvbW1pdHRlZV9pZAAAAAYAAAABAAAAAg==",
        "AAAAAgAAAC5HdWFyZGlhbiBhY3Rpb24gdHlwZSBlbWl0dGVkIGZvciBhdWRpdCB0cmFpbHMuAAAAAAAAAAAAEkd1YXJkaWFuQWN0aW9uS2luZAAAAAAAAgAAAAAAAAAAAAAABFZvaWQAAAAAAAAAAAAAAA5DYW5jZWxQcm9wb3NhbAAA",
        "AAAABQAAAAAAAAAAAAAAEkZhbGxiYWNrU3VwZXJzZWRlZAAAAAAAAQAAABNmYWxsYmFja19zdXBlcnNlZGVkAAAAAAIAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAAAAAAABBwcm9wb3NhbF9vdXRjb21lAAAH0AAAAAdPdXRjb21lAAAAAAAAAAAC",
        "AAAABQAAAAAAAAAAAAAAElJlc29sdXRpb25Qcm9wb3NlZAAAAAAAAQAAABNyZXNvbHV0aW9uX3Byb3Bvc2VkAAAAAAMAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAAAAAAAAdvdXRjb21lAAAAB9AAAAAHT3V0Y29tZQAAAAAAAAAAAAAAAAtwcm9wb3NlZF9hdAAAAAAGAAAAAAAAAAI=",
        "AAAAAQAAAERTdG9yZWQgbGluayBjb25uZWN0aW5nIGFuIGF0dGVzdGF0aW9uIHRvIGEgcmVhc29uaW5nIHJlZ2lzdHJ5IHRyYWNlLgAAAAAAAAATQXR0ZXN0YXRpb25FdmlkZW5jZQAAAAAFAAAAAAAAAAthdHRlc3RlZF9hdAAAAAAGAAAAAAAAAAdvdXRjb21lAAAAB9AAAAAHT3V0Y29tZQAAAAAAAAAABnNpZ25lcgAAAAAAEwAAAAAAAAAKdHJhY2VfaGFzaAAAAAAD7gAAACAAAAAAAAAACHRyYWNlX2lkAAAABg==",
        "AAAABQAAAAAAAAAAAAAAE1Jlc29sdXRpb25GaW5hbGl6ZWQAAAAAAQAAABRyZXNvbHV0aW9uX2ZpbmFsaXplZAAAAAMAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAAAAAAAAdvdXRjb21lAAAAB9AAAAAHT3V0Y29tZQAAAAAAAAAAAAAAAAxmaW5hbGl6ZWRfYXQAAAAGAAAAAAAAAAI=",
        "AAAAAQAAANBBdHRlc3RlZCByZXNvbHV0aW9uIHdob3NlIHByb3Bvc2FsIGNhbiBiZSBjaGFsbGVuZ2VkIHdpdGggYSBib25kIGR1cmluZwp0aGUgZGlzcHV0ZSB3aW5kb3cuIEhhc2hlZCBhcyBgUmVzb2x1dGlvblNwZWM6OlNpZ25lcnNEaXNwdXRhYmxlYCwgYQpzZXBhcmF0ZSB2YXJpYW50LCBzbyBleGlzdGluZyBgU2lnbmVyc2AgY29tbWl0bWVudHMgYXJlIHVuYWZmZWN0ZWQuAAAAAAAAABZEaXNwdXRhYmxlU2lnbmVyQ29uZmlnAAAAAAAGAAAAM0NvbGxhdGVyYWwgZXNjcm93ZWQgYnkgYGRpc3B1dGVgOyBtdXN0IGJlIHBvc2l0aXZlLgAAAAAMZGlzcHV0ZV9ib25kAAAACwAAAAAAAAAOZGlzcHV0ZV93aW5kb3cAAAAAAAYAAAAAAAAACmVzY2FsYXRpb24AAAAAB9AAAAAKRXNjYWxhdGlvbgAAAAAAQ1NlY29uZHMgYWZ0ZXIgYSBkaXNwdXRlIG9wZW5zIGJlZm9yZSBhbnlvbmUgbWF5IHZvaWQgaXQgdW5yZXNvbHZlZC4AAAAAEmVzY2FsYXRpb25fdGltZW91dAAAAAAABgAAAAAAAAAHc2lnbmVycwAAAAPqAAAAEwAAAAAAAAAJdGhyZXNob2xkAAAAAAAABA==",
        "AAAAAgAAAAAAAAAAAAAAB091dGNvbWUAAAAAAgAAAAAAAAAAAAAAA1llcwAAAAAAAAAAAAAAAAJObwAA",
        "AAAAAAAAAAAAAAAFc3RhdGUAAAAAAAABAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAB9AAAAAPUmVzb2x1dGlvblN0YXRlAA==",
        "AAAAAAAAAAAAAAAGYXR0ZXN0AAAAAAADAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAAAAAAZzaWduZXIAAAAAABMAAAAAAAAAB291dGNvbWUAAAAH0AAAAAdPdXRjb21lAAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAi5DaGFsbGVuZ2VzIHRoZSBjdXJyZW50IHByb3Bvc2FsIG9mIGEgYFNpZ25lcnNEaXNwdXRhYmxlYCBtYXJrZXQgd2hpbGUKaXRzIGRpc3B1dGUgd2luZG93IGlzIHN0aWxsIG9wZW4gKHN0cmljdGx5IGJlZm9yZSBgcHJvcG9zZWRfYXQgKwpkaXNwdXRlX3dpbmRvd2AsIHRoZSBpbnN0YW50IGBmaW5hbGl6ZV9zaWduZXJzYCBiZWNvbWVzIGNhbGxhYmxlKS4gVGhlCmRpc3B1dGVyIGVzY3Jvd3MgYGRpc3B1dGVfYm9uZGAgb2YgdGhlIGNvbGxhdGVyYWwgdG9rZW4sIHdoaWNoIGZyZWV6ZXMKZmluYWxpemF0aW9uIHVudGlsIHRoZSBlc2NhbGF0aW9uIGF1dGhvcml0eSBydWxlcyBvcgpgZXNjYWxhdGlvbl90aW1lb3V0YCBsYXBzZXMuIGBhbHRlcm5hdGl2ZWAgaXMgdGhlIG91dGNvbWUgdGhlIGRpc3B1dGVyCmNsYWltcyBpcyBjb3JyZWN0IChgTm9uZWA6IHRoZSBtYXJrZXQgaXMgaW52YWxpZCk7IGl0IG9ubHkgaW5mb3JtcyB0aGUKcnVsaW5nIGFuZCBtdXN0IGRpZmZlciBmcm9tIHRoZSBwcm9wb3NhbC4gVGhlIGRpc3B1dGVyIG1heSBhbHNvIGJlIGEKcHJvcG9zaW5nIHNpZ25lci4AAAAAAAdkaXNwdXRlAAAAAAMAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAACGRpc3B1dGVyAAAAEwAAAAAAAAALYWx0ZXJuYXRpdmUAAAAD6AAAB9AAAAAHT3V0Y29tZQAAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAADdSZWFkcyB0aGUgZXZpZGVuY2UgY2l0YXRpb24gYXQgYGluZGV4YCBmb3IgYG1hcmtldF9pZGAuAAAAAAtldmlkZW5jZV9hdAAAAAACAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAAAAAAVpbmRleAAAAAAAAAQAAAABAAAD6QAAB9AAAAATQXR0ZXN0YXRpb25FdmlkZW5jZQAAAAAD",
        "AAAAAAAAAEZUb3RhbCBwcm9wb3NhbCBjYW5jZWxsYXRpb25zIGV4ZWN1dGVkIGJ5IHRoZSBndWFyZGlhbiBmb3IgYG1hcmtldF9pZGAuAAAAAAAMY2FuY2VsX2NvdW50AAAAAQAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAEAAAAE",
        "AAAAAAAAAAAAAAANX19jb25zdHJ1Y3RvcgAAAAAAAAIAAAAAAAAABWFkbWluAAAAAAAAEwAAAAAAAAALbWFya2V0X2NvcmUAAAAAEwAAAAA=",
        "AAAAAAAAAIFDdXJyZW50IGF0dGVzdGF0aW9uIHJvdW5kIG9mIGEgd2VpZ2h0ZWQta2V5IG1hcmtldDsgdGhlIGByb3VuZGAKYXJndW1lbnQgb2YgYHN1Ym1pdF9zaWduYXR1cmVzYCBhbmQgcGFydCBvZiBldmVyeSBzaWduZWQgbWVzc2FnZS4AAAAAAAANY3VycmVudF9yb3VuZAAAAAAAAAEAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAABA==",
        "AAAAAAAAAfdQZXJtaXNzaW9ubGVzcyBsaXZlbmVzcyBmYWxsYmFjazogb25jZSBgcmVzb2x1dGlvbl9kZWFkbGluZWAgaGFzIHBhc3NlZCwKYW55b25lIG1heSB2b2lkIGEgbWFya2V0IHRoYXQgaXMgc3RpbGwgdW5yZXNvbHZlZCwgc28gY29sbGF0ZXJhbCBpcwpuZXZlciBsb2NrZWQgYmVoaW5kIGEgbWlzc2luZyByZXZlYWwsIGEgZGVhZCBmZWVkIG9yIHNpbGVudCBzaWduZXJzLgoKUmVzb2x1dGlvbiBpcyBwcmVmZXJyZWQgb3ZlciB2b2lkaW5nLiBBIHByaWNlIG1hcmtldCB3aG9zZSBmZWVkIGlzCnZhbGlkIHJpZ2h0IG5vdyBpcyAqcmVzb2x2ZWQqIGluc3RlYWQgKHNvIGEgc3RhbGxlZCBjYWxsZXIgY2FuIG5ldmVyCmZvcmNlIGEgdm9pZCBvbiBhIHJlc29sdmFibGUgbWFya2V0KSwgYW5kIGEgbWFya2V0IHdpdGggYSBwZW5kaW5nCnNpZ25lciBwcm9wb3NhbCBpcyByZWZ1c2VkIHdpdGggYFJlc29sdXRpb25BdmFpbGFibGVgIHVudGlsCmBmaW5hbGl6ZV9zaWduZXJzYCBzZXR0bGVzIGl0LgAAAAANZXhwaXJlX21hcmtldAAAAAAAAAEAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAD6QAAAAIAAAAD",
        "AAAAAAAAADJSZWFkcyBjb21taXR0ZWUgc3RhdGUgYW5kIGV4dGVuZHMgaXRzIHN0b3JhZ2UgVFRMLgAAAAAADWdldF9jb21taXR0ZWUAAAAAAAABAAAAAAAAAAxjb21taXR0ZWVfaWQAAAAGAAAAAQAAA+kAAAfQAAAACUNvbW1pdHRlZQAAAAAAAAM=",
        "AAAAAAAAAFpPcGVucyBmYWxsYmFjayBzaWduZXIgYXR0ZXN0YXRpb24gaW4gYSBoeWJyaWQgbWFya2V0IGlmIHRoZSBmZWVkIGZhaWxlZCBwYXN0IHRoZSBkZWFkbGluZS4AAAAAAA1vcGVuX2ZhbGxiYWNrAAAAAAAAAQAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAAAAAAANcmVzb2x2ZV9wcmljZQAAAAAAAAEAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAD6QAAB9AAAAAHT3V0Y29tZQAAAAAD",
        "AAAAAAAAADJUb3RhbCBldmlkZW5jZSBjaXRhdGlvbnMgcmVjb3JkZWQgZm9yIGBtYXJrZXRfaWRgLgAAAAAADmV2aWRlbmNlX2NvdW50AAAAAAABAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAAAQ=",
        "AAAAAAAAAMFQZXJtaXNzaW9ubGVzcyBmYWxsYmFjayB3aGVuIHRoZSBlc2NhbGF0aW9uIGF1dGhvcml0eSBuZXZlciBydWxlczoKYGVzY2FsYXRpb25fdGltZW91dGAgYWZ0ZXIgdGhlIGRpc3B1dGUgb3BlbmVkLCB0aGUgYm9uZCBpcyByZWZ1bmRlZCBhbmQKdGhlIG1hcmtldCBpcyB2b2lkZWQsIHNvIG5laXRoZXIgc2lkZSB3aW5zIGJ5IGRlZmF1bHQuAAAAAAAADmV4cGlyZV9kaXNwdXRlAAAAAAABAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAADhHdWFyZGlhbiBlc2NhbGF0aW9uOiB0aGUgYWRtaW4gcnVsZXMgb24gYW4gb3BlbiBkaXNwdXRlLgAAAA5zZXR0bGVfZGlzcHV0ZQAAAAAAAgAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAAAAAAHdmVyZGljdAAAAAfQAAAAB1ZlcmRpY3QAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAIZHdWFyZGlhbiBhY3Rpb246IHdpdGhkcmF3cyBhbiB1bmZpbmFsaXplZCBzaWduZXIgcHJvcG9zYWwgZHVyaW5nIHRoZSBkaXNwdXRlIHdpbmRvdwpzdWJqZWN0IHRvIGEgcmF0ZSBsaW1pdCBvZiBNQVhfQ0FOQ0VMU19QRVJfTUFSS0VULgAAAAAAD2NhbmNlbF9wcm9wb3NhbAAAAAACAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAAAAAAZyZWFzb24AAAAAB9AAAAAOR3VhcmRpYW5SZWFzb24AAAAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAADhDYW5jZWxzIGEgcGVuZGluZyByb3RhdGlvbiBmb3IgYSBjb21taXR0ZWUgKGFkbWluIG9ubHkpLgAAAA9jYW5jZWxfcm90YXRpb24AAAAAAQAAAAAAAAAMY29tbWl0dGVlX2lkAAAABgAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAADlSZWFkcyB0aGUgY3VycmVudGx5IGNvbmZpZ3VyZWQgZXZpZGVuY2Ugc2V0dGluZ3MsIGlmIGFueS4AAAAAAAAPZXZpZGVuY2VfY29uZmlnAAAAAAAAAAABAAAD6AAAA+0AAAACAAAAEwAAB9AAAAAORXZpZGVuY2VQb2xpY3kAAA==",
        "AAAAAAAAAFdDcmVhdGVzIGEgcmV1c2FibGUgY29tbWl0dGVlIHdpdGggYW4gYWRtaW4sIG1lbWJlciBzZXQsIHRocmVzaG9sZCwgYW5kIHRpbWUtbG9jayBkZWxheS4AAAAAEGNyZWF0ZV9jb21taXR0ZWUAAAAEAAAAAAAAAAVhZG1pbgAAAAAAABMAAAAAAAAAB21lbWJlcnMAAAAD6gAAABMAAAAAAAAACXRocmVzaG9sZAAAAAAAAAQAAAAAAAAADnJvdGF0aW9uX2RlbGF5AAAAAAAGAAAAAQAAA+kAAAAGAAAAAw==",
        "AAAAAAAAAFBQZXJtaXNzaW9ubGVzc2x5IGV4ZWN1dGVzIGEgbWF0dXJlZCByb3RhdGlvbiBmb3IgYSBjb21taXR0ZWUsIGJ1bXBpbmcgaXRzIGVwb2NoLgAAABBleGVjdXRlX3JvdGF0aW9uAAAAAQAAAAAAAAAMY29tbWl0dGVlX2lkAAAABgAAAAEAAAPpAAAABAAAAAM=",
        "AAAAAAAAAAAAAAAQZmluYWxpemVfc2lnbmVycwAAAAEAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAABAAAD6QAAB9AAAAAHT3V0Y29tZQAAAAAD",
        "AAAAAAAAAEJSZXR1cm5zIHRoZSBndWFyZGlhbiByZWFzb24gY29kZXMgdmFsaWQgZm9yIGBtYXJrZXRfaWRgIHJpZ2h0IG5vdy4AAAAAABBndWFyZGlhbl9vcHRpb25zAAAAAQAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAEAAAPqAAAH0AAAAA5HdWFyZGlhblJlYXNvbgAA",
        "AAAAAAAAAFlQcm9wb3NlcyBhIG5ldyBtZW1iZXIgc2V0IGFuZCB0aHJlc2hvbGQgZm9yIGEgY29tbWl0dGVlLCBleGVjdXRhYmxlIGFmdGVyIHJvdGF0aW9uX2RlbGF5LgAAAAAAABBwcm9wb3NlX3JvdGF0aW9uAAAAAwAAAAAAAAAMY29tbWl0dGVlX2lkAAAABgAAAAAAAAAHbWVtYmVycwAAAAPqAAAAEwAAAAAAAAAJdGhyZXNob2xkAAAAAAAABAAAAAEAAAPpAAAABgAAAAM=",
        "AAAAAAAAAWlSZXZlYWxzIGEgcnVsZSBjb21taXR0ZWQgdG8gdGhyb3VnaCBhIFtgUmVzb2x1dGlvblNwZWNWMmBdIGVudmVsb3BlLgpUaGUgZW52ZWxvcGUgbXVzdCBuYW1lIHRoaXMgbmV0d29yayBhbmQgdGhpcyByZXNvbHZlcidzIG1hcmtldC1jb3JlCihgRG9tYWluTWlzbWF0Y2hgKSwgdXNlIGEgbm9uLXplcm8gc2FsdCBhbmQgdGhlIGN1cnJlbnQgYHZlcnNpb25gCihgSW52YWxpZENvbmZpZ2ApLCBhbmQgaGFzaCB0byB0aGUgbWFya2V0J3MgY29tbWl0bWVudAooYFNwZWNIYXNoTWlzbWF0Y2hgKS4gVGhlIGlubmVyIHJ1bGUgaXMgdGhlbiB2YWxpZGF0ZWQgYW5kIHN0b3JlZApleGFjdGx5IGFzIHRoZSBsZWdhY3kgZW50cnlwb2ludHMgZG8uAAAAAAAAEHJlZ2lzdGVyX3NwZWNfdjIAAAACAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAAAAAARzcGVjAAAH0AAAABBSZXNvbHV0aW9uU3BlY1YyAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAGhDb21taXR0ZWUgZXNjYWxhdGlvbjogZWFjaCBtZW1iZXIgdm90ZXMgb25jZTsgdGhlIGZpcnN0IHZlcmRpY3QgdG8KZ2F0aGVyIGBrYCB2b3RlcyBzZXR0bGVzIHRoZSBkaXNwdXRlLgAAABFlc2NhbGF0aW9uX2F0dGVzdAAAAAAAAAMAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAABm1lbWJlcgAAAAAAEwAAAAAAAAAHdmVyZGljdAAAAAfQAAAAB1ZlcmRpY3QAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAhRDb3VudHMgRWQyNTUxOSBzaWduYXR1cmVzIGZyb20gYSB3ZWlnaHRlZCBjb21taXR0ZWUuIFBlcm1pc3Npb25sZXNzOiB0aGUKc2lnbmF0dXJlcywgbm90IHRoZSBzdWJtaXR0ZXIsIGNhcnJ5IHRoZSBhdXRob3JpdHkuIEVhY2ggc2lnbmF0dXJlIGlzCm92ZXIgdGhlIG1lc3NhZ2UgYnVpbHQgYnkgYGF0dGVzdGF0aW9uX21lc3NhZ2VgOyBub24tbWVtYmVycyBhbmQga2V5cwp0aGF0IGFscmVhZHkgc2lnbmVkIHRoaXMgcm91bmQgKGZvciBlaXRoZXIgb3V0Y29tZSkgYXJlIHNraXBwZWQsIGFuZCBhCnNpbmdsZSBpbnZhbGlkIHNpZ25hdHVyZSBhYm9ydHMgdGhlIHdob2xlIGJhdGNoLCBzbyBzdWJtaXR0ZXJzIHNob3VsZApwcmUtZmlsdGVyIHdpdGggYSB2ZXJpZmllci4gT25jZSB0aGUgYWNjdW11bGF0ZWQgd2VpZ2h0IGZvciBgb3V0Y29tZWAKZmlyc3QgcmVhY2hlcyB0aGUgdGhyZXNob2xkIGEgcHJvcG9zYWwgaXMgcmVjb3JkZWQgYW5kIHRoZSBkaXNwdXRlCndpbmRvdyBzdGFydHM7IGxhdGVyIGNhbGxzIG5ldmVyIG1vdmUgaXQuAAAAEXN1Ym1pdF9zaWduYXR1cmVzAAAAAAAABAAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAAAAAAHb3V0Y29tZQAAAAfQAAAAB091dGNvbWUAAAAAAAAAAAVyb3VuZAAAAAAAAAQAAAAAAAAABHNpZ3MAAAPqAAAD7QAAAAIAAAPuAAAAIAAAA+4AAABAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAIRQYXlzIGEgc2V0dGxlZCBkaXNwdXRlJ3MgYm9uZCB0byBpdHMgcGF5ZWUgKHRoZSBkaXNwdXRlciwgb3IgdGhlCm1hcmtldC1jb3JlIHRyZWFzdXJ5IHdoZW4gdGhlIHByb3Bvc2FsIHdhcyB1cGhlbGQpLiBQZXJtaXNzaW9ubGVzcy4AAAASY2xhaW1fZGlzcHV0ZV9ib25kAAAAAAABAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAATpSZXZlYWxzIHRoZSByZXNvbHV0aW9uIHJ1bGUgY29tbWl0dGVkIHRvIGF0IG1hcmtldCBjcmVhdGlvbi4gQW55b25lCm1heSBjYWxsIHRoaXMgKHRoZXJlIGlzIG5vIGFkbWluIGdhdGUpIGJlY2F1c2UgdGhlIG9ubHkgdGhpbmcgdGhhdAptYWtlcyBhIHNwZWMgYWNjZXB0YWJsZSBpcyB0aGF0IGl0IGhhc2hlcyB0byBgbWFya2V0LnJlc29sdXRpb25faGFzaGAK4oCUIHRoZSBjb21taXRtZW50LCBub3QgdGhlIGNhbGxlciwgaXMgdGhlIGF1dGhvcml0eS4gRmFpbHMgaWYgYSBzcGVjCndhcyBhbHJlYWR5IHJlZ2lzdGVyZWQgZm9yIHRoaXMgbWFya2V0LgAAAAAAE3JlZ2lzdGVyX3ByaWNlX3NwZWMAAAAAAgAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAAAAAAEc3BlYwAAB9AAAAALUHJpY2VDb25maWcAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAL1XaGVuIGBleHBpcmVfbWFya2V0YCBiZWNvbWVzIGNhbGxhYmxlIGZvciBgbWFya2V0X2lkYDogYGNsb3NlX3RpbWVgIHBsdXMKW2BSRVNPTFVUSU9OX0dSQUNFX1NFQ1NgXS4gTmVlZHMgb25seSBtYXJrZXQtY29yZSdzIGBjbG9zZV90aW1lYCwgc28gaXQKaXMga25vd24gYmVmb3JlIChhbmQgd2l0aG91dCkgYSBzcGVjIHJldmVhbC4AAAAAAAATcmVzb2x1dGlvbl9kZWFkbGluZQAAAAABAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAQAAA+kAAAAGAAAAAw==",
        "AAAAAAAAAFRDb25maWd1cmVzIHRoZSByZWFzb25pbmcgcmVnaXN0cnkgY29udHJhY3QgYWRkcmVzcyBhbmQgZXZpZGVuY2UgcG9saWN5IChhZG1pbiBvbmx5KS4AAAATc2V0X2V2aWRlbmNlX2NvbmZpZwAAAAACAAAAAAAAAAhyZWdpc3RyeQAAABMAAAAAAAAABnBvbGljeQAAAAAH0AAAAA5FdmlkZW5jZVBvbGljeQAAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAFJBdHRlc3RzIHRvIGFuIG91dGNvbWUgYmFja2VkIGJ5IGFuIG9uLWNoYWluIHJlYXNvbmluZyB0cmFjZSBpbiByZWFzb25pbmctcmVnaXN0cnkuAAAAAAAUYXR0ZXN0X3dpdGhfZXZpZGVuY2UAAAAEAAAAAAAAAAltYXJrZXRfaWQAAAAAAAAGAAAAAAAAAAZzaWduZXIAAAAAABMAAAAAAAAAB291dGNvbWUAAAAH0AAAAAdPdXRjb21lAAAAAAAAAAAIdHJhY2VfaWQAAAAGAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAFxHdWFyZGlhbiBhY3Rpb246IHZvaWRzIGEgbWFya2V0IHN1YmplY3QgdG8gdmVyaWZpZWQgb24tY2hhaW4gcmVhc29uIGNvZGVzIGFuZCBncmFjZSBwZXJpb2RzLgAAABRndWFyZGlhbl92b2lkX21hcmtldAAAAAIAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAABnJlYXNvbgAAAAAH0AAAAA5HdWFyZGlhblJlYXNvbgAAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAFBSZWdpc3RlcnMgYSBoeWJyaWQgcmVzb2x1dGlvbiBzcGVjaWZpY2F0aW9uIChwcmljZSBmZWVkIHdpdGggYXR0ZXN0ZWQgZmFsbGJhY2spLgAAABRyZWdpc3Rlcl9oeWJyaWRfc3BlYwAAAAIAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAABHNwZWMAAAfQAAAADEh5YnJpZENvbmZpZwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAENTZWUgYHJlZ2lzdGVyX3ByaWNlX3NwZWNgIOKAlCBzYW1lIGNvbW1pdC9yZXZlYWwgcnVsZSwgc2lnbmVyIG1vZGUuAAAAABRyZWdpc3Rlcl9zaWduZXJfc3BlYwAAAAIAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAABHNwZWMAAAfQAAAADFNpZ25lckNvbmZpZwAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAD1SZWdpc3RlcnMgYSByZXVzYWJsZSBjb21taXR0ZWUgc3BlY2lmaWNhdGlvbiBmb3IgYG1hcmtldF9pZGAuAAAAAAAAF3JlZ2lzdGVyX2NvbW1pdHRlZV9zcGVjAAAAAAIAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAABHNwZWMAAAfQAAAADENvbW1pdHRlZVJlZgAAAAEAAAPpAAAAAgAAAAM=",
        "AAAAAAAAAIFTZWUgYHJlZ2lzdGVyX3ByaWNlX3NwZWNgIOKAlCBzYW1lIGNvbW1pdC9yZXZlYWwgcnVsZSwgc2lnbmVyIG1vZGUgd2hvc2UKcHJvcG9zYWxzIGNhbiBiZSBjaGFsbGVuZ2VkIHdpdGggYSBib25kIChzZWUgYGRpc3B1dGVgKS4AAAAAAAAYcmVnaXN0ZXJfZGlzcHV0YWJsZV9zcGVjAAAAAgAAAAAAAAAJbWFya2V0X2lkAAAAAAAABgAAAAAAAAAEc3BlYwAAB9AAAAAWRGlzcHV0YWJsZVNpZ25lckNvbmZpZwAAAAAAAQAAA+kAAAACAAAAAw==",
        "AAAAAAAAAI5TZWUgYHJlZ2lzdGVyX3ByaWNlX3NwZWNgIOKAlCBzYW1lIGNvbW1pdC9yZXZlYWwgcnVsZSwgd2VpZ2h0ZWQgRWQyNTUxOQpjb21taXR0ZWUgdGhhdCBzaWducyBvZmYtY2hhaW4gYW5kIGlzIHZlcmlmaWVkIGJ5IGBzdWJtaXRfc2lnbmF0dXJlc2AuAAAAAAAacmVnaXN0ZXJfd2VpZ2h0ZWRfa2V5X3NwZWMAAAAAAAIAAAAAAAAACW1hcmtldF9pZAAAAAAAAAYAAAAAAAAABHNwZWMAAAfQAAAAEVdlaWdodGVkS2V5Q29uZmlnAAAAAAAAAQAAA+kAAAACAAAAAw==" ]),
      options
    )
  }
  public readonly fromJSON = {
    state: this.txFromJSON<ResolutionState>,
        attest: this.txFromJSON<Result<void>>,
        dispute: this.txFromJSON<Result<void>>,
        evidence_at: this.txFromJSON<Result<AttestationEvidence>>,
        cancel_count: this.txFromJSON<u32>,
        current_round: this.txFromJSON<u32>,
        expire_market: this.txFromJSON<Result<void>>,
        get_committee: this.txFromJSON<Result<Committee>>,
        open_fallback: this.txFromJSON<Result<void>>,
        resolve_price: this.txFromJSON<Result<Outcome>>,
        evidence_count: this.txFromJSON<u32>,
        expire_dispute: this.txFromJSON<Result<void>>,
        settle_dispute: this.txFromJSON<Result<void>>,
        cancel_proposal: this.txFromJSON<Result<void>>,
        cancel_rotation: this.txFromJSON<Result<void>>,
        evidence_config: this.txFromJSON<Option<readonly [string, EvidencePolicy]>>,
        create_committee: this.txFromJSON<Result<u64>>,
        execute_rotation: this.txFromJSON<Result<u32>>,
        finalize_signers: this.txFromJSON<Result<Outcome>>,
        guardian_options: this.txFromJSON<Array<GuardianReason>>,
        propose_rotation: this.txFromJSON<Result<u64>>,
        register_spec_v2: this.txFromJSON<Result<void>>,
        escalation_attest: this.txFromJSON<Result<void>>,
        submit_signatures: this.txFromJSON<Result<void>>,
        claim_dispute_bond: this.txFromJSON<Result<void>>,
        register_price_spec: this.txFromJSON<Result<void>>,
        resolution_deadline: this.txFromJSON<Result<u64>>,
        set_evidence_config: this.txFromJSON<Result<void>>,
        attest_with_evidence: this.txFromJSON<Result<void>>,
        guardian_void_market: this.txFromJSON<Result<void>>,
        register_hybrid_spec: this.txFromJSON<Result<void>>,
        register_signer_spec: this.txFromJSON<Result<void>>,
        register_committee_spec: this.txFromJSON<Result<void>>,
        register_disputable_spec: this.txFromJSON<Result<void>>,
        register_weighted_key_spec: this.txFromJSON<Result<void>>
  }
}