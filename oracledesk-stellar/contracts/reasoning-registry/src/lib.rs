#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, Bytes, BytesN,
    Env, String, Symbol, Vec,
};

mod market_core {
    #[allow(clippy::too_many_arguments)]
    soroban_sdk::contractimport!(file = "market_core.wasm");
}

/// Domain tag for commit-reveal commitments (#51). Must match
/// `agents/core/trace.ts` byte-for-byte: `sha256(TAG || trace_hash || salt)`.
pub const COMMIT_DOMAIN_TAG: &[u8] = b"oracledesk.trace.commit.v1";

/// Merkle tree constants for batch anchoring (#54).
pub const MAX_LEAVES: u32 = 4096;
pub const MAX_PROOF_LEN: u32 = 12;
pub const MERKLE_LEAF_PREFIX: u8 = 0x00;
pub const MERKLE_NODE_PREFIX: u8 = 0x01;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceBatch {
    pub agent: Address,
    pub root: BytesN<32>,
    pub leaf_count: u32,
    pub manifest_cid: String,
    pub published_at: u64,
}

const BUMP_THRESHOLD: u32 = 30 * 17_280;
const BUMP_TO: u32 = 60 * 17_280;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceAction {
    CreateMarket,
    ProposeMarket,
    Buy,
    Sell,
    Skip,
    Redeem,
    ClaimRemainder,
    Resolve,
    Other(Symbol),
}

pub const NO_MARKET: u64 = u64::MAX;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AgentStatus {
    Active,
    Retired(u64),
    Revoked(u64, Symbol),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRecord {
    pub status: AgentStatus,
    pub registered_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFlag {
    pub reason: Symbol,
    pub flagged_at: u64,
    pub active: bool,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TraceVerdict {
    Valid,
    HashMismatch,
    Flagged(Symbol),
    AgentRevokedAfter(u64),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trace {
    pub market_id: u64,
    pub agent: Address,
    pub action: TraceAction,
    pub trace_hash: BytesN<32>,
    pub ipfs_cid: String,
    pub published_at: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    NextId,
    Agent(Address),
    Trace(u64),
    /// Number of traces published for a market, so a client can page through
    /// `MarketTrace(market_id, 0..count)` without the contract ever holding
    /// an unbounded Vec of trace ids in one storage entry.
    MarketTraceCount(u64),
    MarketTrace(u64, u64),
    /// Ledger sequence a trace was written in (#50). Kept beside `Trace`
    /// rather than inside it so the `Trace` XDR shape doesn't change.
    TraceLedger(u64),
    /// Contracts allowed to call `consume_trace` (#50).
    Consumer(Address),
    /// Set once a trace has backed an action (#50).
    Consumed(u64),
    /// Per-agent hash-chain head (#52).
    AgentHead(Address),
    /// Per-agent `seq -> trace_id` (#52).
    AgentTrace(Address, u64),
    /// `trace_id -> ChainLink`; absent for legacy traces (#52).
    TraceLink(u64),
    /// Active trace flag annotation (#56).
    Flag(u64),
    /// Number of flag history entries for a trace (#56).
    FlagHistoryCount(u64),
    /// Flag history entries: (trace_id, index) -> TraceFlag (#56).
    FlagHistory(u64, u32),
    /// Address of market-core contract (#55).
    MarketCore,
    /// Merkle trace batch: batch_id -> TraceBatch (#54).
    Batch(u64),
    /// Next batch ID (#54).
    NextBatchId,
    /// Number of batches anchored by agent (#54).
    AgentBatchCount(Address),
    /// Agent batch index: (agent, index) -> batch_id (#54).
    AgentBatch(Address, u64),
}

/// Head of an agent's append-only trace log (#52). Genesis is `seq = 0`
/// with an all-zero hash.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainHead {
    pub seq: u64,
    pub head_hash: BytesN<32>,
    pub head_trace_id: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainLink {
    pub seq: u64,
    pub prev_trace_hash: BytesN<32>,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 1,
    Unauthorized = 2,
    AgentAlreadyRegistered = 3,
    AgentNotRegistered = 4,
    TraceNotFound = 5,
    /// Retained for ABI compatibility; actions are now validated via `TraceAction` (#55).
    EmptyAction = 6,
    EmptyCid = 7,
    /// `expected_seq` / `prev_trace_hash` don't extend the agent's chain head (#52).
    ChainHeadMismatch = 8,
    /// Agent already has a chain, so unchained `publish_trace` is refused (#52).
    ChainedAgentMustChain = 9,
    /// Trace already backed an action (#50).
    TraceAlreadyConsumed = 10,
    /// Agent is retired or revoked (#56).
    AgentNotActive = 11,
    /// A revoked agent cannot be reactivated (#56).
    CannotReactivateRevoked = 12,
    /// Trace flag history exceeds maximum 16 entries (#56).
    FlagHistoryFull = 13,
    /// Revocation effective_from is invalid (#56).
    InvalidEffectiveTime = 14,
    /// market_id has not been issued by market-core (#55).
    UnknownMarket = 15,
    /// Action requires a valid market_id instead of NO_MARKET (#55).
    MarketRequired = 16,
    /// ProposeMarket action must use NO_MARKET sentinel (#55).
    ProposalMustUseNoMarket = 17,
    /// Other(Symbol) shadows a canonical TraceAction name (#55).
    UseCanonicalAction = 18,
    /// Batch does not exist (#54).
    BatchNotFound = 19,
    /// Batch parameters are invalid (leaf_count out of bounds or empty manifest) (#54).
    InvalidBatch = 20,
    /// Inclusion proof is invalid or malformed (#54).
    InvalidProof = 21,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRetired {
    #[topic]
    pub agent: Address,
    pub at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRevoked {
    #[topic]
    pub agent: Address,
    pub effective_from: u64,
    pub reason: Symbol,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentReactivated {
    #[topic]
    pub agent: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceFlagged {
    #[topic]
    pub trace_id: u64,
    pub reason: Symbol,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceUnflagged {
    #[topic]
    pub trace_id: u64,
    pub reason: Symbol,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceChained {
    #[topic]
    pub agent: Address,
    #[topic]
    pub seq: u64,
    pub trace_id: u64,
    pub prev_trace_hash: BytesN<32>,
    pub trace_hash: BytesN<32>,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumerRegistered {
    #[topic]
    pub consumer: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsumerUnregistered {
    #[topic]
    pub consumer: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceConsumed {
    #[topic]
    pub trace_id: u64,
    pub consumer: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRegistered {
    #[topic]
    pub agent: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TracePublished {
    #[topic]
    pub trace_id: u64,
    #[topic]
    pub market_id: u64,
    #[topic]
    pub agent: Address,
    pub action: TraceAction,
    pub trace_hash: BytesN<32>,
    pub ipfs_cid: String,
    pub published_at: u64,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchPublished {
    #[topic]
    pub batch_id: u64,
    #[topic]
    pub agent: Address,
    pub root: BytesN<32>,
    pub leaf_count: u32,
    pub manifest_cid: String,
    pub published_at: u64,
}

#[contract]
pub struct ReasoningRegistry;

#[contractimpl]
impl ReasoningRegistry {
    pub fn __constructor(env: Env, admin: Address, market_core: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::MarketCore, &market_core);
        env.storage().instance().set(&DataKey::NextId, &0u64);
        env.storage().instance().set(&DataKey::NextBatchId, &0u64);
    }

    pub fn register_agent(env: Env, agent: Address) -> Result<(), Error> {
        Self::require_admin(&env)?;
        if Self::load_agent(&env, &agent).is_some() {
            return Err(Error::AgentAlreadyRegistered);
        }
        let now = env.ledger().timestamp();
        let record = AgentRecord {
            status: AgentStatus::Active,
            registered_at: now,
        };
        Self::put(&env, &DataKey::Agent(agent.clone()), &record);
        AgentRegistered { agent }.publish(&env);
        Ok(())
    }

    pub fn retire_agent(env: Env, agent: Address) -> Result<(), Error> {
        Self::require_admin(&env)?;
        Self::retire_agent_internal(&env, agent)
    }

    pub fn retire_self(env: Env, agent: Address) -> Result<(), Error> {
        agent.require_auth();
        Self::retire_agent_internal(&env, agent)
    }

    pub fn reactivate_agent(env: Env, agent: Address) -> Result<(), Error> {
        Self::require_admin(&env)?;
        let mut record = Self::load_agent(&env, &agent).ok_or(Error::AgentNotRegistered)?;
        match record.status {
            AgentStatus::Retired(_) => {
                record.status = AgentStatus::Active;
                Self::put(&env, &DataKey::Agent(agent.clone()), &record);
                AgentReactivated { agent }.publish(&env);
                Ok(())
            }
            AgentStatus::Revoked(_, _) => Err(Error::CannotReactivateRevoked),
            AgentStatus::Active => Err(Error::AgentAlreadyRegistered),
        }
    }

    pub fn revoke_agent(
        env: Env,
        agent: Address,
        effective_from: u64,
        reason: Symbol,
    ) -> Result<(), Error> {
        Self::require_admin(&env)?;
        let mut record = Self::load_agent(&env, &agent).ok_or(Error::AgentNotRegistered)?;
        let now = env.ledger().timestamp();
        if effective_from < record.registered_at || effective_from > now {
            return Err(Error::InvalidEffectiveTime);
        }
        record.status = AgentStatus::Revoked(effective_from, reason.clone());
        Self::put(&env, &DataKey::Agent(agent.clone()), &record);
        AgentRevoked {
            agent,
            effective_from,
            reason,
        }
        .publish(&env);
        Ok(())
    }

    pub fn unregister_agent(env: Env, agent: Address) -> Result<(), Error> {
        Self::retire_agent(env, agent)
    }

    pub fn is_registered(env: Env, agent: Address) -> bool {
        Self::load_agent(&env, &agent).map_or(false, |r| matches!(r.status, AgentStatus::Active))
    }

    pub fn get_agent(env: Env, agent: Address) -> Option<AgentRecord> {
        Self::load_agent(&env, &agent)
    }

    pub fn publish_trace(
        env: Env,
        agent: Address,
        market_id: u64,
        action: TraceAction,
        trace_hash: BytesN<32>,
        ipfs_cid: String,
    ) -> Result<u64, Error> {
        Self::check_publish(&env, &agent, market_id, &action, &ipfs_cid)?;
        // Once an agent keeps a chained log, unchained traces would sit outside
        // it unseen, so they're refused (#52).
        if Self::load_head(&env, &agent).seq > 0 {
            return Err(Error::ChainedAgentMustChain);
        }
        Self::write_trace(&env, agent, market_id, action, trace_hash, ipfs_cid)
    }

    /// Publishes a trace that extends `agent`'s hash chain (#52). Fails with
    /// `ChainHeadMismatch` unless `expected_seq == head.seq + 1` and
    /// `prev_trace_hash == head.head_hash`, so a skipped decision shows up as
    /// a gap and a fork is impossible.
    #[allow(clippy::too_many_arguments)]
    pub fn publish_chained_trace(
        env: Env,
        agent: Address,
        market_id: u64,
        action: TraceAction,
        trace_hash: BytesN<32>,
        ipfs_cid: String,
        expected_seq: u64,
        prev_trace_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        Self::check_publish(&env, &agent, market_id, &action, &ipfs_cid)?;
        let head = Self::load_head(&env, &agent);
        if expected_seq != head.seq + 1 || prev_trace_hash != head.head_hash {
            return Err(Error::ChainHeadMismatch);
        }

        let id = Self::write_trace(
            &env,
            agent.clone(),
            market_id,
            action,
            trace_hash.clone(),
            ipfs_cid,
        )?;

        let link_key = DataKey::TraceLink(id);
        Self::put(
            &env,
            &link_key,
            &ChainLink {
                seq: expected_seq,
                prev_trace_hash: prev_trace_hash.clone(),
            },
        );
        Self::put(&env, &DataKey::AgentTrace(agent.clone(), expected_seq), &id);
        Self::put(
            &env,
            &DataKey::AgentHead(agent.clone()),
            &ChainHead {
                seq: expected_seq,
                head_hash: trace_hash.clone(),
                head_trace_id: Some(id),
            },
        );

        TraceChained {
            agent,
            seq: expected_seq,
            trace_id: id,
            prev_trace_hash,
            trace_hash,
        }
        .publish(&env);
        Ok(id)
    }

    /// Current chain head for `agent` (genesis if it has never chained).
    pub fn agent_chain_head(env: Env, agent: Address) -> ChainHead {
        Self::load_head(&env, &agent)
    }

    /// Trace id at position `seq` (1-based) of `agent`'s chain.
    pub fn agent_trace_at(env: Env, agent: Address, seq: u64) -> Result<u64, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::AgentTrace(agent, seq))
            .ok_or(Error::TraceNotFound)
    }

    /// Chain link for a trace; `None` for legacy, unchained traces.
    pub fn get_trace_link(env: Env, trace_id: u64) -> Option<ChainLink> {
        env.storage()
            .persistent()
            .get(&DataKey::TraceLink(trace_id))
    }

    /// Ledger sequence the trace was written in (#50). `None` for traces
    /// written before this field existed.
    pub fn get_trace_ledger(env: Env, trace_id: u64) -> Option<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::TraceLedger(trace_id))
    }

    /// Allow-lists a contract (e.g. the treasury) to call `consume_trace` (#50).
    pub fn register_consumer(env: Env, consumer: Address) -> Result<(), Error> {
        Self::require_admin(&env)?;
        Self::put(&env, &DataKey::Consumer(consumer.clone()), &true);
        ConsumerRegistered { consumer }.publish(&env);
        Ok(())
    }

    pub fn unregister_consumer(env: Env, consumer: Address) -> Result<(), Error> {
        Self::require_admin(&env)?;
        env.storage()
            .persistent()
            .remove(&DataKey::Consumer(consumer.clone()));
        ConsumerUnregistered { consumer }.publish(&env);
        Ok(())
    }

    pub fn is_consumer(env: Env, consumer: Address) -> bool {
        env.storage().persistent().has(&DataKey::Consumer(consumer))
    }

    /// Marks a trace as having backed an action, exactly once (#50). Only an
    /// allow-listed consumer contract can call it; the treasury passes its own
    /// address, which satisfies `require_auth` as the direct invoker.
    pub fn consume_trace(env: Env, consumer: Address, trace_id: u64) -> Result<Trace, Error> {
        consumer.require_auth();
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Consumer(consumer.clone()))
        {
            return Err(Error::Unauthorized);
        }
        let trace = Self::get_trace(env.clone(), trace_id)?;
        let key = DataKey::Consumed(trace_id);
        if env.storage().persistent().has(&key) {
            return Err(Error::TraceAlreadyConsumed);
        }
        Self::put(&env, &key, &true);
        TraceConsumed { trace_id, consumer }.publish(&env);
        Ok(trace)
    }

    pub fn is_consumed(env: Env, trace_id: u64) -> bool {
        env.storage().persistent().has(&DataKey::Consumed(trace_id))
    }

    /// Checks a commit-reveal commitment (#51):
    /// `commitment == sha256("oracledesk.trace.commit.v1" || trace_hash || salt)`.
    pub fn verify_commitment(
        env: Env,
        trace_hash: BytesN<32>,
        salt: BytesN<32>,
        commitment: BytesN<32>,
    ) -> bool {
        Self::commitment_for(&env, &trace_hash, &salt) == commitment
    }

    pub fn get_trace(env: Env, trace_id: u64) -> Result<Trace, Error> {
        let key = DataKey::Trace(trace_id);
        let trace: Trace = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::TraceNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
        Ok(trace)
    }

    pub fn flag_trace(env: Env, trace_id: u64, reason: Symbol) -> Result<(), Error> {
        Self::require_admin(&env)?;
        Self::get_trace(env.clone(), trace_id)?;
        let count_key = DataKey::FlagHistoryCount(trace_id);
        let count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
        if count >= 16 {
            return Err(Error::FlagHistoryFull);
        }
        let now = env.ledger().timestamp();
        let flag = TraceFlag {
            reason: reason.clone(),
            flagged_at: now,
            active: true,
        };
        Self::put(&env, &DataKey::Flag(trace_id), &flag);
        Self::put(&env, &DataKey::FlagHistory(trace_id, count), &flag);
        Self::put(&env, &count_key, &(count + 1));
        TraceFlagged { trace_id, reason }.publish(&env);
        Ok(())
    }

    pub fn unflag_trace(env: Env, trace_id: u64, reason: Symbol) -> Result<(), Error> {
        Self::require_admin(&env)?;
        Self::get_trace(env.clone(), trace_id)?;
        let count_key = DataKey::FlagHistoryCount(trace_id);
        let count: u32 = env.storage().persistent().get(&count_key).unwrap_or(0);
        if count >= 16 {
            return Err(Error::FlagHistoryFull);
        }
        let now = env.ledger().timestamp();
        let flag = TraceFlag {
            reason: reason.clone(),
            flagged_at: now,
            active: false,
        };
        Self::put(&env, &DataKey::Flag(trace_id), &flag);
        Self::put(&env, &DataKey::FlagHistory(trace_id, count), &flag);
        Self::put(&env, &count_key, &(count + 1));
        TraceUnflagged { trace_id, reason }.publish(&env);
        Ok(())
    }

    pub fn trace_flags(
        env: Env,
        trace_id: u64,
        start: u32,
        limit: u32,
    ) -> Result<Vec<TraceFlag>, Error> {
        Self::get_trace(env.clone(), trace_id)?;
        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::FlagHistoryCount(trace_id))
            .unwrap_or(0);
        let mut list = Vec::new(&env);
        let end = (start + limit).min(count);
        for i in start..end {
            if let Some(flag) = env
                .storage()
                .persistent()
                .get::<_, TraceFlag>(&DataKey::FlagHistory(trace_id, i))
            {
                list.push_back(flag);
            }
        }
        Ok(list)
    }

    pub fn get_trace_flag(env: Env, trace_id: u64) -> Option<TraceFlag> {
        env.storage().persistent().get(&DataKey::Flag(trace_id))
    }

    pub fn trace_verdict(
        env: Env,
        trace_id: u64,
        received_hash: BytesN<32>,
    ) -> Result<TraceVerdict, Error> {
        let trace = Self::get_trace(env.clone(), trace_id)?;
        if trace.trace_hash != received_hash {
            return Ok(TraceVerdict::HashMismatch);
        }
        if let Some(flag) = env
            .storage()
            .persistent()
            .get::<_, TraceFlag>(&DataKey::Flag(trace_id))
        {
            if flag.active {
                return Ok(TraceVerdict::Flagged(flag.reason));
            }
        }
        if let Some(record) = Self::load_agent(&env, &trace.agent) {
            if let AgentStatus::Revoked(effective_from, _) = record.status {
                if trace.published_at >= effective_from {
                    return Ok(TraceVerdict::AgentRevokedAfter(effective_from));
                }
            }
        }
        Ok(TraceVerdict::Valid)
    }

    pub fn verify_trace(env: Env, trace_id: u64, received_hash: BytesN<32>) -> Result<bool, Error> {
        Ok(Self::trace_verdict(env, trace_id, received_hash)? == TraceVerdict::Valid)
    }

    pub fn trace_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::NextId).unwrap_or(0)
    }

    /// Number of traces published for `market_id`.
    pub fn market_trace_count(env: Env, market_id: u64) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::MarketTraceCount(market_id))
            .unwrap_or(0)
    }

    /// The `index`-th (0-based, publish order) trace id for `market_id`.
    pub fn market_trace_at(env: Env, market_id: u64, index: u64) -> Result<u64, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::MarketTrace(market_id, index))
            .ok_or(Error::TraceNotFound)
    }

    /// Anchors a Merkle batch of N traces off-chain, storing only the 32-byte root
    /// and manifest CID on-chain (#54).
    pub fn publish_trace_batch(
        env: Env,
        agent: Address,
        root: BytesN<32>,
        leaf_count: u32,
        manifest_cid: String,
    ) -> Result<u64, Error> {
        agent.require_auth();
        let record = Self::load_agent(&env, &agent).ok_or(Error::AgentNotRegistered)?;
        if !matches!(record.status, AgentStatus::Active) {
            return Err(Error::AgentNotActive);
        }
        if leaf_count == 0 || leaf_count > MAX_LEAVES || manifest_cid.is_empty() {
            return Err(Error::InvalidBatch);
        }

        let batch_id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextBatchId)
            .ok_or(Error::NotInitialized)?;
        env.storage().instance().set(&DataKey::NextBatchId, &(batch_id + 1));

        let batch = TraceBatch {
            agent: agent.clone(),
            root: root.clone(),
            leaf_count,
            manifest_cid: manifest_cid.clone(),
            published_at: env.ledger().timestamp(),
        };
        Self::put(&env, &DataKey::Batch(batch_id), &batch);

        let agent_count_key = DataKey::AgentBatchCount(agent.clone());
        let agent_idx: u64 = env.storage().persistent().get(&agent_count_key).unwrap_or(0);
        Self::put(&env, &DataKey::AgentBatch(agent.clone(), agent_idx), &batch_id);
        Self::put(&env, &agent_count_key, &(agent_idx + 1));

        BatchPublished {
            batch_id,
            agent,
            root,
            leaf_count,
            manifest_cid,
            published_at: batch.published_at,
        }
        .publish(&env);

        Ok(batch_id)
    }

    pub fn get_batch(env: Env, batch_id: u64) -> Result<TraceBatch, Error> {
        let key = DataKey::Batch(batch_id);
        let batch: TraceBatch = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::BatchNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
        Ok(batch)
    }

    pub fn batch_count(env: Env) -> u64 {
        env.storage().instance().get(&DataKey::NextBatchId).unwrap_or(0)
    }

    pub fn agent_batch_count(env: Env, agent: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::AgentBatchCount(agent))
            .unwrap_or(0)
    }

    pub fn agent_batch_at(env: Env, agent: Address, index: u64) -> Result<u64, Error> {
        let key = DataKey::AgentBatch(agent, index);
        let id: u64 = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(Error::BatchNotFound)?;
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
        Ok(id)
    }

    pub fn verify_trace_inclusion(
        env: Env,
        batch_id: u64,
        trace_hash: BytesN<32>,
        index: u32,
        proof: Vec<BytesN<32>>,
    ) -> Result<bool, Error> {
        let batch = Self::get_batch(env.clone(), batch_id)?;
        if index >= batch.leaf_count {
            return Ok(false);
        }
        if proof.len() > MAX_PROOF_LEN {
            return Err(Error::InvalidProof);
        }
        match Self::compute_merkle_root(&env, batch.leaf_count, &trace_hash, index, &proof) {
            Ok(computed) => Ok(computed == batch.root),
            Err(Error::InvalidProof) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn compute_merkle_root(
        env: &Env,
        leaf_count: u32,
        trace_hash: &BytesN<32>,
        index: u32,
        proof: &Vec<BytesN<32>>,
    ) -> Result<BytesN<32>, Error> {
        let mut leaf_preimage = Bytes::new(env);
        leaf_preimage.push_back(MERKLE_LEAF_PREFIX);
        leaf_preimage.append(&trace_hash.clone().into());
        let mut current: BytesN<32> = env.crypto().sha256(&leaf_preimage).into();

        let mut curr_idx = index;
        let mut curr_len = leaf_count;
        let mut proof_idx: u32 = 0;

        while curr_len > 1 {
            if curr_len % 2 == 1 && curr_idx == curr_len - 1 {
                // Odd last node promoted unchanged: no sibling in proof
            } else {
                if proof_idx >= proof.len() {
                    return Err(Error::InvalidProof);
                }
                let sibling = proof.get(proof_idx).unwrap();
                proof_idx += 1;

                let mut node_preimage = Bytes::new(env);
                node_preimage.push_back(MERKLE_NODE_PREFIX);
                if curr_idx % 2 == 0 {
                    node_preimage.append(&current.clone().into());
                    node_preimage.append(&sibling.into());
                } else {
                    node_preimage.append(&sibling.into());
                    node_preimage.append(&current.clone().into());
                }
                current = env.crypto().sha256(&node_preimage).into();
            }
            curr_idx /= 2;
            curr_len = (curr_len + 1) / 2;
        }

        if proof_idx != proof.len() {
            return Err(Error::InvalidProof);
        }

        Ok(current)
    }
}

impl ReasoningRegistry {
    fn retire_agent_internal(env: &Env, agent: Address) -> Result<(), Error> {
        let mut record = Self::load_agent(env, &agent).ok_or(Error::AgentNotRegistered)?;
        if matches!(record.status, AgentStatus::Revoked(_, _)) {
            return Err(Error::CannotReactivateRevoked);
        }
        let now = env.ledger().timestamp();
        record.status = AgentStatus::Retired(now);
        Self::put(env, &DataKey::Agent(agent.clone()), &record);
        AgentRetired { agent, at: now }.publish(env);
        Ok(())
    }

    fn load_agent(env: &Env, addr: &Address) -> Option<AgentRecord> {
        let key = DataKey::Agent(addr.clone());
        if let Some(record) = env.storage().persistent().get::<_, AgentRecord>(&key) {
            env.storage()
                .persistent()
                .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
            Some(record)
        } else if let Some(is_reg) = env.storage().persistent().get::<_, bool>(&key) {
            if is_reg {
                env.storage()
                    .persistent()
                    .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
                Some(AgentRecord {
                    status: AgentStatus::Active,
                    registered_at: 0,
                })
            } else {
                None
            }
        } else {
            None
        }
    }

    fn validate_action(env: &Env, action: &TraceAction) -> Result<(), Error> {
        if let TraceAction::Other(sym) = action {
            let s = sym.clone();
            if s == Symbol::new(env, "create_market")
                || s == Symbol::new(env, "propose_market")
                || s == Symbol::new(env, "buy")
                || s == Symbol::new(env, "sell")
                || s == Symbol::new(env, "skip")
                || s == Symbol::new(env, "redeem")
                || s == Symbol::new(env, "claim_remainder")
                || s == Symbol::new(env, "resolve")
            {
                return Err(Error::UseCanonicalAction);
            }
        }
        Ok(())
    }

    fn validate_reference(env: &Env, market_id: u64, action: &TraceAction) -> Result<(), Error> {
        let pre_creation = matches!(action, TraceAction::ProposeMarket);
        if market_id == NO_MARKET {
            return if pre_creation || matches!(action, TraceAction::Other(_)) {
                Ok(())
            } else {
                Err(Error::MarketRequired)
            };
        }
        if pre_creation {
            return Err(Error::ProposalMustUseNoMarket);
        }
        let mc: Address = env
            .storage()
            .instance()
            .get(&DataKey::MarketCore)
            .ok_or(Error::NotInitialized)?;
        let count = market_core::Client::new(env, &mc).market_count();
        if market_id >= count {
            return Err(Error::UnknownMarket);
        }
        Ok(())
    }

    fn check_publish(
        env: &Env,
        agent: &Address,
        market_id: u64,
        action: &TraceAction,
        ipfs_cid: &String,
    ) -> Result<(), Error> {
        agent.require_auth();
        let record = Self::load_agent(env, agent).ok_or(Error::AgentNotRegistered)?;
        if !matches!(record.status, AgentStatus::Active) {
            return Err(Error::AgentNotActive);
        }
        Self::validate_action(env, action)?;
        Self::validate_reference(env, market_id, action)?;
        if ipfs_cid.is_empty() {
            return Err(Error::EmptyCid);
        }
        Ok(())
    }

    /// Shared body of every publish path: allocates the id, stores the trace,
    /// its ledger and the per-market index, and emits `TracePublished`.
    fn write_trace(
        env: &Env,
        agent: Address,
        market_id: u64,
        action: TraceAction,
        trace_hash: BytesN<32>,
        ipfs_cid: String,
    ) -> Result<u64, Error> {
        let id: u64 = env
            .storage()
            .instance()
            .get(&DataKey::NextId)
            .ok_or(Error::NotInitialized)?;
        env.storage().instance().set(&DataKey::NextId, &(id + 1));

        let trace = Trace {
            market_id,
            agent: agent.clone(),
            action: action.clone(),
            trace_hash: trace_hash.clone(),
            ipfs_cid: ipfs_cid.clone(),
            published_at: env.ledger().timestamp(),
        };
        Self::put(env, &DataKey::Trace(id), &trace);
        Self::put(env, &DataKey::TraceLedger(id), &env.ledger().sequence());

        let count_key = DataKey::MarketTraceCount(market_id);
        let index: u64 = env.storage().persistent().get(&count_key).unwrap_or(0);
        Self::put(env, &DataKey::MarketTrace(market_id, index), &id);
        Self::put(env, &count_key, &(index + 1));

        TracePublished {
            trace_id: id,
            market_id,
            agent,
            action,
            trace_hash,
            ipfs_cid,
            published_at: trace.published_at,
        }
        .publish(env);
        Ok(id)
    }

    /// Loads (and keeps alive) an agent's chain head. Never reset by
    /// unregister/re-register, so history can't be rewritten (#52).
    fn load_head(env: &Env, agent: &Address) -> ChainHead {
        let key = DataKey::AgentHead(agent.clone());
        match env.storage().persistent().get::<_, ChainHead>(&key) {
            Some(head) => {
                env.storage()
                    .persistent()
                    .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
                head
            }
            None => ChainHead {
                seq: 0,
                head_hash: BytesN::from_array(env, &[0u8; 32]),
                head_trace_id: None,
            },
        }
    }

    fn put<V: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, key: &DataKey, value: &V) {
        env.storage().persistent().set(key, value);
        env.storage()
            .persistent()
            .extend_ttl(key, BUMP_THRESHOLD, BUMP_TO);
    }

    fn commitment_for(env: &Env, trace_hash: &BytesN<32>, salt: &BytesN<32>) -> BytesN<32> {
        let mut bytes = Bytes::from_slice(env, COMMIT_DOMAIN_TAG);
        bytes.append(&trace_hash.clone().into());
        bytes.append(&salt.clone().into());
        env.crypto().sha256(&bytes).into()
    }

    fn require_admin(env: &Env) -> Result<(), Error> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        admin.require_auth();
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _, MockAuth, MockAuthInvoke},
        IntoVal,
    };

    fn register_market_core(env: &Env, admin: &Address) -> (Address, market_core::Client<'static>) {
        let treasury = Address::generate(env);
        let resolver = Address::generate(env);
        let creator = Address::generate(env);

        let collateral = env.register_stellar_asset_contract_v2(admin.clone());
        let token = soroban_sdk::token::StellarAssetClient::new(env, &collateral.address());
        token.mint(&creator, &(50_000 * 10_000_000_i128));

        let market_core_id = env.register(
            market_core::WASM,
            (
                admin.clone(),
                collateral.address(),
                treasury,
                resolver,
                100u32,
                10_000u32,
            ),
        );
        let market_core = market_core::Client::new(env, &market_core_id);
        for _ in 0..50 {
            market_core.create_market(
                &creator,
                &BytesN::from_array(env, &[42u8; 32]),
                &BytesN::from_array(env, &[142u8; 32]),
                &String::from_str(env, "ipfs://market"),
                &market_core::Category::Crypto,
                &(env.ledger().timestamp() + 60_000),
                &(100 * 10_000_000_i128),
                &5000u32,
            );
        }
        (market_core_id, market_core)
    }

    fn setup() -> (Env, ReasoningRegistryClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let agent = Address::generate(&env);
        let (market_core_id, _) = register_market_core(&env, &admin);
        let contract_id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin, &market_core_id),
        );
        let client = ReasoningRegistryClient::new(&env, &contract_id);
        (env, client, admin, agent)
    }

    #[test]
    fn registered_agent_can_publish_and_buyer_can_verify_hash() {
        let (env, client, admin, agent) = setup();
        client.register_agent(&agent);

        let trace_hash = BytesN::from_array(&env, &[7u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &42,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://trace-42"),
        );

        assert_eq!(trace_id, 0);
        assert_eq!(client.trace_count(), 1);
        assert!(client.verify_trace(&trace_id, &trace_hash));
        assert!(!client.verify_trace(&trace_id, &BytesN::from_array(&env, &[8u8; 32]),));

        let trace = client.get_trace(&trace_id);
        assert_eq!(trace.market_id, 42);
        assert_eq!(trace.agent, agent);
        assert_eq!(trace.action, TraceAction::Buy);
        assert_eq!(trace.ipfs_cid, String::from_str(&env, "ipfs://trace-42"));
        assert!(client.is_registered(&agent));

        client.unregister_agent(&agent);
        assert!(!client.is_registered(&agent));
        let record = client.get_agent(&agent).unwrap();
        assert!(matches!(record.status, AgentStatus::Retired(_)));
        assert!(client.verify_trace(&trace_id, &trace_hash));
        let _ = admin;
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #4)")]
    fn unregistered_agent_cannot_publish() {
        let (env, client, _, agent) = setup();
        client.publish_trace(
            &agent,
            &1,
            &TraceAction::Skip,
            &BytesN::from_array(&env, &[1u8; 32]),
            &String::from_str(&env, "ipfs://unauthorized"),
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #5)")]
    fn unknown_trace_cannot_be_verified() {
        let (env, client, _, _) = setup();
        client.verify_trace(&99, &BytesN::from_array(&env, &[0u8; 32]));
    }

    #[test]
    fn traces_are_indexed_per_market() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);

        let t0 = client.publish_trace(
            &agent,
            &7,
            &TraceAction::CreateMarket,
            &BytesN::from_array(&env, &[1u8; 32]),
            &String::from_str(&env, "ipfs://t0"),
        );
        let t1 = client.publish_trace(
            &agent,
            &7,
            &TraceAction::Buy,
            &BytesN::from_array(&env, &[2u8; 32]),
            &String::from_str(&env, "ipfs://t1"),
        );
        // A trace on a different market must not appear in market 7's index.
        client.publish_trace(
            &agent,
            &8,
            &TraceAction::Buy,
            &BytesN::from_array(&env, &[3u8; 32]),
            &String::from_str(&env, "ipfs://t2"),
        );

        assert_eq!(client.market_trace_count(&7), 2);
        assert_eq!(client.market_trace_at(&7, &0), t0);
        assert_eq!(client.market_trace_at(&7, &1), t1);
        assert_eq!(client.market_trace_count(&8), 1);
    }

    #[test]
    fn wrong_caller_cannot_register_agents() {
        let (env, client, _, agent) = setup();
        let not_admin = Address::generate(&env);
        env.set_auths(&[]);
        assert!(client
            .mock_auths(&[MockAuth {
                address: &not_admin,
                invoke: &MockAuthInvoke {
                    contract: &client.address,
                    fn_name: "register_agent",
                    args: (agent.clone(),).into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_register_agent(&agent)
            .is_err());
    }

    #[test]
    fn agent_cannot_publish_trace_impersonating_another_agent() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let impostor = Address::generate(&env);
        env.set_auths(&[]);
        // `impostor` authorizes itself, but the call claims `agent` published
        // the trace; only `agent`'s own signature may satisfy that.
        assert!(client
            .mock_auths(&[MockAuth {
                address: &impostor,
                invoke: &MockAuthInvoke {
                    contract: &client.address,
                    fn_name: "publish_trace",
                    args: (
                        agent.clone(),
                        1u64,
                        TraceAction::Buy,
                        BytesN::from_array(&env, &[9u8; 32]),
                        String::from_str(&env, "ipfs://impostor"),
                    )
                        .into_val(&env),
                    sub_invokes: &[],
                },
            }])
            .try_publish_trace(
                &agent,
                &1,
                &TraceAction::Buy,
                &BytesN::from_array(&env, &[9u8; 32]),
                &String::from_str(&env, "ipfs://impostor"),
            )
            .is_err());
    }

    // ── #52 hash-chained traces ──────────────────────────────────────────

    fn chained(
        env: &Env,
        client: &ReasoningRegistryClient,
        agent: &Address,
        seq: u64,
        prev: &BytesN<32>,
        hash: u8,
    ) -> Result<u64, Error> {
        client
            .try_publish_chained_trace(
                agent,
                &1,
                &TraceAction::Buy,
                &BytesN::from_array(env, &[hash; 32]),
                &String::from_str(env, "ipfs://chain"),
                &seq,
                prev,
            )
            .map(|r| r.unwrap())
            .map_err(|e| e.unwrap())
    }

    #[test]
    fn first_chained_trace_requires_genesis_head() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let zero = BytesN::from_array(&env, &[0u8; 32]);
        assert_eq!(
            chained(&env, &client, &agent, 2, &zero, 1),
            Err(Error::ChainHeadMismatch)
        );
        assert_eq!(
            chained(
                &env,
                &client,
                &agent,
                1,
                &BytesN::from_array(&env, &[5u8; 32]),
                1
            ),
            Err(Error::ChainHeadMismatch)
        );
        assert_eq!(chained(&env, &client, &agent, 1, &zero, 1), Ok(0));
    }

    #[test]
    fn chain_extends_sequentially_and_views_return_ids() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let zero = BytesN::from_array(&env, &[0u8; 32]);
        let id1 = chained(&env, &client, &agent, 1, &zero, 1).unwrap();
        let id2 = chained(
            &env,
            &client,
            &agent,
            2,
            &BytesN::from_array(&env, &[1u8; 32]),
            2,
        )
        .unwrap();

        let head = client.agent_chain_head(&agent);
        assert_eq!(head.seq, 2);
        assert_eq!(head.head_hash, BytesN::from_array(&env, &[2u8; 32]));
        assert_eq!(head.head_trace_id, Some(id2));
        assert_eq!(client.agent_trace_at(&agent, &1), id1);
        assert_eq!(client.agent_trace_at(&agent, &2), id2);
        assert_eq!(
            client.get_trace_link(&id2),
            Some(ChainLink {
                seq: 2,
                prev_trace_hash: BytesN::from_array(&env, &[1u8; 32])
            })
        );
    }

    #[test]
    fn stale_head_publish_is_rejected() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let zero = BytesN::from_array(&env, &[0u8; 32]);
        chained(&env, &client, &agent, 1, &zero, 1).unwrap();
        // A racing publisher that read the same genesis head loses.
        assert_eq!(
            chained(&env, &client, &agent, 1, &zero, 9),
            Err(Error::ChainHeadMismatch)
        );
    }

    #[test]
    fn wrong_prev_hash_is_rejected() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        chained(
            &env,
            &client,
            &agent,
            1,
            &BytesN::from_array(&env, &[0u8; 32]),
            1,
        )
        .unwrap();
        assert_eq!(
            chained(
                &env,
                &client,
                &agent,
                2,
                &BytesN::from_array(&env, &[3u8; 32]),
                2
            ),
            Err(Error::ChainHeadMismatch)
        );
    }

    #[test]
    fn legacy_publish_blocked_after_chain_started() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        chained(
            &env,
            &client,
            &agent,
            1,
            &BytesN::from_array(&env, &[0u8; 32]),
            1,
        )
        .unwrap();
        let res = client.try_publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &BytesN::from_array(&env, &[4u8; 32]),
            &String::from_str(&env, "ipfs://legacy"),
        );
        assert_eq!(res, Err(Ok(Error::ChainedAgentMustChain)));
    }

    #[test]
    fn legacy_publish_still_works_for_unchained_agent() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let id = client.publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &BytesN::from_array(&env, &[4u8; 32]),
            &String::from_str(&env, "ipfs://legacy"),
        );
        assert_eq!(client.get_trace_link(&id), None);
        assert_eq!(client.agent_chain_head(&agent).seq, 0);
    }

    #[test]
    fn reregistration_does_not_reset_chain_head() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        chained(
            &env,
            &client,
            &agent,
            1,
            &BytesN::from_array(&env, &[0u8; 32]),
            1,
        )
        .unwrap();
        client.unregister_agent(&agent);
        client.register_agent(&agent);
        assert_eq!(client.agent_chain_head(&agent).seq, 1);
        assert_eq!(
            chained(
                &env,
                &client,
                &agent,
                1,
                &BytesN::from_array(&env, &[0u8; 32]),
                7
            ),
            Err(Error::ChainHeadMismatch)
        );
    }

    #[test]
    fn impostor_cannot_extend_another_agents_chain() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let agent = Address::generate(&env);
        let impostor = Address::generate(&env);
        let (market_core_id, _) = register_market_core(&env, &admin);
        let id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin, &market_core_id),
        );
        let client = ReasoningRegistryClient::new(&env, &id);
        env.mock_all_auths();
        client.register_agent(&agent);

        let action = TraceAction::Buy;
        let hash = BytesN::from_array(&env, &[1u8; 32]);
        let cid = String::from_str(&env, "ipfs://x");
        let zero = BytesN::from_array(&env, &[0u8; 32]);
        env.mock_auths(&[MockAuth {
            address: &impostor,
            invoke: &MockAuthInvoke {
                contract: &id,
                fn_name: "publish_chained_trace",
                args: (&agent, 1u64, &action, &hash, &cid, 1u64, &zero).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        assert!(client
            .try_publish_chained_trace(&agent, &1, &action, &hash, &cid, &1, &zero)
            .is_err());
        assert_eq!(client.agent_chain_head(&agent).seq, 0);
    }

    // ── #50 trace consumption + ledger ───────────────────────────────────

    #[test]
    fn trace_records_ledger_and_is_consumed_once() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        env.ledger().set_sequence_number(77);
        let id = client.publish_trace(
            &agent,
            &3,
            &TraceAction::Buy,
            &BytesN::from_array(&env, &[6u8; 32]),
            &String::from_str(&env, "ipfs://intent"),
        );
        assert_eq!(client.get_trace_ledger(&id), Some(77));

        let treasury = Address::generate(&env);
        client.register_consumer(&treasury);
        assert!(client.is_consumer(&treasury));
        let trace = client.consume_trace(&treasury, &id);
        assert_eq!(trace.market_id, 3);
        assert!(client.is_consumed(&id));
        assert_eq!(
            client.try_consume_trace(&treasury, &id),
            Err(Ok(Error::TraceAlreadyConsumed))
        );
    }

    #[test]
    fn only_registered_consumer_can_consume() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let id = client.publish_trace(
            &agent,
            &3,
            &TraceAction::Buy,
            &BytesN::from_array(&env, &[6u8; 32]),
            &String::from_str(&env, "ipfs://intent"),
        );
        let stranger = Address::generate(&env);
        assert_eq!(
            client.try_consume_trace(&stranger, &id),
            Err(Ok(Error::Unauthorized))
        );

        client.register_consumer(&stranger);
        client.unregister_consumer(&stranger);
        assert_eq!(
            client.try_consume_trace(&stranger, &id),
            Err(Ok(Error::Unauthorized))
        );
        assert!(!client.is_consumed(&id));
    }

    #[test]
    fn wrong_caller_cannot_register_consumer() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let (market_core_id, _) = register_market_core(&env, &admin);
        let id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin, &market_core_id),
        );
        let client = ReasoningRegistryClient::new(&env, &id);
        let attacker = Address::generate(&env);
        let consumer = Address::generate(&env);
        env.mock_auths(&[MockAuth {
            address: &attacker,
            invoke: &MockAuthInvoke {
                contract: &id,
                fn_name: "register_consumer",
                args: (&consumer,).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        assert!(client.try_register_consumer(&consumer).is_err());
        assert!(!client.is_consumer(&consumer));
    }

    // ── #51 commit-reveal commitment ─────────────────────────────────────

    #[test]
    fn commitment_vector_matches_typescript() {
        let (env, client, _, _) = setup();
        let hash = BytesN::from_array(&env, &[7u8; 32]);
        let salt = BytesN::from_array(&env, &[9u8; 32]);
        // Same vector as agents/core/trace.test.ts.
        let expected: [u8; 32] = [
            0x50, 0xa6, 0x30, 0xb8, 0x3f, 0x3b, 0x4e, 0x9c, 0x44, 0x42, 0x65, 0xcc, 0x6a, 0xf2,
            0xa0, 0x94, 0xbf, 0xf7, 0xe3, 0x84, 0x8a, 0x33, 0x4f, 0x01, 0x67, 0xfb, 0x54, 0x9d,
            0x6c, 0xa7, 0xee, 0x52,
        ];
        assert!(client.verify_commitment(&hash, &salt, &BytesN::from_array(&env, &expected)));
        assert!(!client.verify_commitment(
            &hash,
            &BytesN::from_array(&env, &[8u8; 32]),
            &BytesN::from_array(&env, &expected)
        ));
    }

    // ── #56 Trace lifecycle: retirement, revocation, and flagging ─────────

    #[test]
    fn retired_agent_cannot_publish_but_old_traces_valid() {
        let (env, client, admin, agent) = setup();
        client.register_agent(&agent);

        let trace_hash = BytesN::from_array(&env, &[1u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://t1"),
        );

        client.retire_agent(&agent);
        assert!(!client.is_registered(&agent));

        assert_eq!(
            client.try_publish_trace(
                &agent,
                &1,
                &TraceAction::Buy,
                &BytesN::from_array(&env, &[2u8; 32]),
                &String::from_str(&env, "ipfs://t2"),
            ),
            Err(Ok(Error::AgentNotActive))
        );

        assert_eq!(
            client.trace_verdict(&trace_id, &trace_hash),
            TraceVerdict::Valid
        );
        assert!(client.verify_trace(&trace_id, &trace_hash));

        client.reactivate_agent(&agent);
        assert!(client.is_registered(&agent));
        let trace_id2 = client.publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &BytesN::from_array(&env, &[3u8; 32]),
            &String::from_str(&env, "ipfs://t3"),
        );
        assert_eq!(trace_id2, 1);
        let _ = admin;
    }

    #[test]
    fn revoked_agent_traces_after_effective_time_report_revoked() {
        let (env, client, _admin, agent) = setup();
        env.ledger().set_timestamp(100);
        client.register_agent(&agent);

        env.ledger().set_timestamp(200);
        let trace_hash = BytesN::from_array(&env, &[1u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://t1"),
        );

        env.ledger().set_timestamp(300);
        client.revoke_agent(&agent, &150, &Symbol::new(&env, "compromised"));

        assert_eq!(
            client.trace_verdict(&trace_id, &trace_hash),
            TraceVerdict::AgentRevokedAfter(150)
        );
        assert!(!client.verify_trace(&trace_id, &trace_hash));
    }

    #[test]
    fn revoked_agent_traces_before_effective_time_remain_valid() {
        let (env, client, _admin, agent) = setup();
        env.ledger().set_timestamp(100);
        client.register_agent(&agent);

        env.ledger().set_timestamp(120);
        let trace_hash = BytesN::from_array(&env, &[1u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://t1"),
        );

        env.ledger().set_timestamp(200);
        client.revoke_agent(&agent, &150, &Symbol::new(&env, "compromised"));

        assert_eq!(
            client.trace_verdict(&trace_id, &trace_hash),
            TraceVerdict::Valid
        );
        assert!(client.verify_trace(&trace_id, &trace_hash));
    }

    #[test]
    fn revoked_agent_cannot_be_reregistered_or_reactivated() {
        let (env, client, _admin, agent) = setup();
        env.ledger().set_timestamp(100);
        client.register_agent(&agent);
        env.ledger().set_timestamp(200);
        client.revoke_agent(&agent, &150, &Symbol::new(&env, "compromised"));

        assert_eq!(
            client.try_register_agent(&agent),
            Err(Ok(Error::AgentAlreadyRegistered))
        );
        assert_eq!(
            client.try_reactivate_agent(&agent),
            Err(Ok(Error::CannotReactivateRevoked))
        );
    }

    #[test]
    fn flag_and_unflag_append_history_never_mutate_trace() {
        let (env, client, _admin, agent) = setup();
        client.register_agent(&agent);
        let trace_hash = BytesN::from_array(&env, &[1u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://t1"),
        );

        let initial_trace = client.get_trace(&trace_id);

        client.flag_trace(&trace_id, &Symbol::new(&env, "suspicious"));
        assert_eq!(
            client.trace_verdict(&trace_id, &trace_hash),
            TraceVerdict::Flagged(Symbol::new(&env, "suspicious"))
        );

        assert_eq!(client.get_trace(&trace_id), initial_trace);

        client.unflag_trace(&trace_id, &Symbol::new(&env, "cleared"));
        assert_eq!(
            client.trace_verdict(&trace_id, &trace_hash),
            TraceVerdict::Valid
        );

        assert_eq!(client.get_trace(&trace_id), initial_trace);

        let flags = client.trace_flags(&trace_id, &0, &10);
        assert_eq!(flags.len(), 2);
        assert!(flags.get(0).unwrap().active);
        assert_eq!(flags.get(0).unwrap().reason, Symbol::new(&env, "suspicious"));
        assert!(!flags.get(1).unwrap().active);
        assert_eq!(flags.get(1).unwrap().reason, Symbol::new(&env, "cleared"));
    }

    #[test]
    fn flag_history_is_bounded() {
        let (env, client, _admin, agent) = setup();
        client.register_agent(&agent);
        let trace_hash = BytesN::from_array(&env, &[1u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://t1"),
        );

        for _ in 0..8 {
            client.flag_trace(&trace_id, &Symbol::new(&env, "flag"));
            client.unflag_trace(&trace_id, &Symbol::new(&env, "unflag"));
        }

        assert_eq!(
            client.try_flag_trace(&trace_id, &Symbol::new(&env, "flag17")),
            Err(Ok(Error::FlagHistoryFull))
        );
    }

    #[test]
    fn verify_trace_false_for_flagged_trace() {
        let (env, client, _admin, agent) = setup();
        client.register_agent(&agent);
        let trace_hash = BytesN::from_array(&env, &[1u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &1,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://t1"),
        );

        assert!(client.verify_trace(&trace_id, &trace_hash));
        client.flag_trace(&trace_id, &Symbol::new(&env, "fraud"));
        assert!(!client.verify_trace(&trace_id, &trace_hash));
    }

    #[test]
    fn legacy_bool_agent_record_reads_as_active() {
        let (env, client, _admin, agent) = setup();
        env.as_contract(&client.address, || {
            env.storage()
                .persistent()
                .set(&DataKey::Agent(agent.clone()), &true);
        });
        assert!(client.is_registered(&agent));
        let record = client.get_agent(&agent).unwrap();
        assert_eq!(record.status, AgentStatus::Active);
        assert_eq!(record.registered_at, 0);
    }

    #[test]
    fn wrong_caller_cannot_flag_or_revoke() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let agent = Address::generate(&env);
        let (market_core_id, _) = register_market_core(&env, &admin);
        let contract_id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin, &market_core_id),
        );
        let client = ReasoningRegistryClient::new(&env, &contract_id);

        env.mock_auths(&[MockAuth {
            address: &admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "register_agent",
                args: (&agent,).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        client.register_agent(&agent);

        let trace_hash = BytesN::from_array(&env, &[7u8; 32]);
        env.mock_auths(&[MockAuth {
            address: &agent,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "publish_trace",
                args: (
                    &agent,
                    &42u64,
                    &TraceAction::Buy,
                    &trace_hash,
                    &String::from_str(&env, "ipfs://trace-42"),
                )
                    .into_val(&env),
                sub_invokes: &[],
            },
        }]);
        let trace_id = client.publish_trace(
            &agent,
            &42,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://trace-42"),
        );

        let non_admin = Address::generate(&env);
        env.mock_auths(&[MockAuth {
            address: &non_admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "flag_trace",
                args: (&trace_id, &Symbol::new(&env, "bad_trade")).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        assert!(client.try_flag_trace(&trace_id, &Symbol::new(&env, "bad_trade")).is_err());

        env.mock_auths(&[MockAuth {
            address: &non_admin,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "revoke_agent",
                args: (&agent, &0u64, &Symbol::new(&env, "compromised")).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        assert!(client.try_revoke_agent(&agent, &0u64, &Symbol::new(&env, "compromised")).is_err());
    }

    #[test]
    fn agent_can_retire_itself_but_not_others() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let agent1 = Address::generate(&env);
        let agent2 = Address::generate(&env);
        let (market_core_id, _) = register_market_core(&env, &admin);
        let contract_id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin, &market_core_id),
        );
        let client = ReasoningRegistryClient::new(&env, &contract_id);

        env.mock_all_auths();
        client.register_agent(&agent1);
        client.register_agent(&agent2);

        env.mock_auths(&[MockAuth {
            address: &agent1,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "retire_self",
                args: (&agent1,).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        client.retire_self(&agent1);
        assert!(!client.is_registered(&agent1));

        env.mock_auths(&[MockAuth {
            address: &agent1,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "retire_self",
                args: (&agent2,).into_val(&env),
                sub_invokes: &[],
            },
        }]);
        assert!(client.try_retire_self(&agent2).is_err());
    }

    // ── #55 Trace market references and action validation ────────────────

    #[test]
    fn publish_for_existing_market_succeeds() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let trace_hash = BytesN::from_array(&env, &[11u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &0,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://trace-0"),
        );
        let trace = client.get_trace(&trace_id);
        assert_eq!(trace.market_id, 0);
        assert_eq!(trace.action, TraceAction::Buy);
    }

    #[test]
    fn publish_for_unknown_market_fails_unknown_market() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let trace_hash = BytesN::from_array(&env, &[12u8; 32]);
        // 50 markets exist (0..49), so market 50 and 9_999_999 are unknown
        assert_eq!(
            client.try_publish_trace(
                &agent,
                &50,
                &TraceAction::Buy,
                &trace_hash,
                &String::from_str(&env, "ipfs://trace-unknown"),
            ),
            Err(Ok(Error::UnknownMarket))
        );
        assert_eq!(
            client.try_publish_trace(
                &agent,
                &9_999_999,
                &TraceAction::Buy,
                &trace_hash,
                &String::from_str(&env, "ipfs://trace-unknown"),
            ),
            Err(Ok(Error::UnknownMarket))
        );
    }

    #[test]
    fn propose_market_requires_no_market_sentinel() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let trace_hash = BytesN::from_array(&env, &[13u8; 32]);
        // ProposeMarket with an actual market_id must fail with ProposalMustUseNoMarket
        assert_eq!(
            client.try_publish_trace(
                &agent,
                &1,
                &TraceAction::ProposeMarket,
                &trace_hash,
                &String::from_str(&env, "ipfs://trace-propose"),
            ),
            Err(Ok(Error::ProposalMustUseNoMarket))
        );
        // ProposeMarket with NO_MARKET must succeed
        let id = client.publish_trace(
            &agent,
            &NO_MARKET,
            &TraceAction::ProposeMarket,
            &trace_hash,
            &String::from_str(&env, "ipfs://trace-propose"),
        );
        let trace = client.get_trace(&id);
        assert_eq!(trace.market_id, NO_MARKET);
        assert_eq!(trace.action, TraceAction::ProposeMarket);
    }

    #[test]
    fn no_market_sentinel_rejected_for_buy() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let trace_hash = BytesN::from_array(&env, &[14u8; 32]);
        assert_eq!(
            client.try_publish_trace(
                &agent,
                &NO_MARKET,
                &TraceAction::Buy,
                &trace_hash,
                &String::from_str(&env, "ipfs://trace-buy-nomarket"),
            ),
            Err(Ok(Error::MarketRequired))
        );
    }

    #[test]
    fn other_action_cannot_shadow_canonical_name() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let trace_hash = BytesN::from_array(&env, &[15u8; 32]);
        // Attempting to use Other with canonical names
        let canonical_names = [
            "create_market",
            "propose_market",
            "buy",
            "sell",
            "skip",
            "redeem",
            "claim_remainder",
            "resolve",
        ];
        for name in canonical_names.iter() {
            assert_eq!(
                client.try_publish_trace(
                    &agent,
                    &0,
                    &TraceAction::Other(Symbol::new(&env, name)),
                    &trace_hash,
                    &String::from_str(&env, "ipfs://trace-shadow"),
                ),
                Err(Ok(Error::UseCanonicalAction))
            );
        }
        // Non-canonical name succeeds (works with NO_MARKET or valid market_id)
        let id1 = client.publish_trace(
            &agent,
            &NO_MARKET,
            &TraceAction::Other(Symbol::new(&env, "custom_strategy")),
            &trace_hash,
            &String::from_str(&env, "ipfs://trace-custom"),
        );
        let trace1 = client.get_trace(&id1);
        assert_eq!(trace1.action, TraceAction::Other(Symbol::new(&env, "custom_strategy")));

        let id2 = client.publish_trace(
            &agent,
            &0,
            &TraceAction::Other(Symbol::new(&env, "custom_strategy_2")),
            &trace_hash,
            &String::from_str(&env, "ipfs://trace-custom-2"),
        );
        let trace2 = client.get_trace(&id2);
        assert_eq!(trace2.action, TraceAction::Other(Symbol::new(&env, "custom_strategy_2")));
    }

    #[test]
    fn publish_works_while_market_core_paused() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let agent = Address::generate(&env);
        let (market_core_id, market_core_client) = register_market_core(&env, &admin);
        let contract_id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin, &market_core_id),
        );
        let client = ReasoningRegistryClient::new(&env, &contract_id);
        client.register_agent(&agent);

        // Pause market-core
        market_core_client.set_paused(&true);

        // Registry publish_trace must still succeed because market_count is a view call
        let trace_hash = BytesN::from_array(&env, &[16u8; 32]);
        let trace_id = client.publish_trace(
            &agent,
            &0,
            &TraceAction::Buy,
            &trace_hash,
            &String::from_str(&env, "ipfs://trace-while-paused"),
        );
        assert_eq!(trace_id, 0);
    }

    #[test]
    fn market_ids_are_dense_assumption_holds() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let (_market_core_id, market_core_client) = register_market_core(&env, &admin);
        let count = market_core_client.market_count();
        assert_eq!(count, 50);
    }

    // ── #54 Merkle-batched trace anchoring ──────────────────────────────

    #[test]
    fn merkle_vector_sizes_match_spec() {
        let (env, client, _, _) = setup();

        // Size 1: leaf 0
        let trace_0 = BytesN::from_array(
            &env,
            &[
                0x63, 0x6e, 0xef, 0xad, 0x93, 0xfd, 0xbc, 0x03, 0xa2, 0x58, 0x52, 0x2c, 0x63, 0x67,
                0x2a, 0xf9, 0x1a, 0x5f, 0x67, 0x62, 0x4e, 0x9b, 0xc9, 0x31, 0xbe, 0xe4, 0xf4, 0x37,
                0x4e, 0x1f, 0xab, 0x58,
            ],
        );
        let root_1 = BytesN::from_array(
            &env,
            &[
                0xd2, 0xea, 0x33, 0x68, 0xe7, 0x8c, 0x8f, 0xf7, 0x63, 0xb5, 0x92, 0x3a, 0x59, 0xf8,
                0xaf, 0x10, 0x1d, 0xda, 0x4c, 0x3b, 0xff, 0x46, 0xae, 0x74, 0xe1, 0xe9, 0xa5, 0x2c,
                0x77, 0x55, 0xaa, 0xeb,
            ],
        );
        let proof_1: Vec<BytesN<32>> = Vec::new(&env);
        let computed_1 = client.compute_merkle_root(&1, &trace_0, &0, &proof_1);
        assert_eq!(computed_1, root_1);

        // Size 2: leaf 0 with leaf 1 sibling
        let root_2 = BytesN::from_array(
            &env,
            &[
                0xcd, 0x3a, 0x97, 0xb9, 0x45, 0x78, 0x20, 0x45, 0x0f, 0xf6, 0xd5, 0x77, 0xb6, 0xdd,
                0x8a, 0x13, 0xbe, 0xf9, 0x4a, 0xce, 0x08, 0xb8, 0xec, 0xba, 0xbe, 0x4e, 0x2c, 0xa4,
                0x74, 0x71, 0xe2, 0x29,
            ],
        );
        let sibling_1 = BytesN::from_array(
            &env,
            &[
                0x17, 0x82, 0x46, 0xc1, 0x01, 0xf8, 0x2f, 0xba, 0x9b, 0x1b, 0xb6, 0x30, 0x3f, 0x1e,
                0x9f, 0x13, 0x12, 0x04, 0x26, 0x55, 0x65, 0x76, 0xc5, 0xeb, 0xc7, 0x8c, 0x42, 0xd7,
                0x6a, 0x73, 0x48, 0xb3,
            ],
        );
        let mut proof_2: Vec<BytesN<32>> = Vec::new(&env);
        proof_2.push_back(sibling_1);
        let computed_2 = client.compute_merkle_root(&2, &trace_0, &0, &proof_2);
        assert_eq!(computed_2, root_2);

        // Size 3: leaf 2 (odd node promoted to level 1, then paired with root_2)
        let trace_2 = BytesN::from_array(
            &env,
            &[
                0x42, 0x4d, 0xa8, 0xb6, 0x08, 0x8f, 0xd9, 0xf1, 0xc2, 0xd0, 0xd1, 0x1c, 0x5a, 0x6c,
                0xd5, 0xd6, 0x1b, 0xf5, 0x31, 0x6d, 0xee, 0x8e, 0x61, 0xb3, 0xb0, 0xa3, 0x0f, 0x2a,
                0x01, 0xb8, 0xb4, 0xa1,
            ],
        );
        let root_3 = BytesN::from_array(
            &env,
            &[
                0xa5, 0xb9, 0x47, 0x06, 0xba, 0xa6, 0x8c, 0x77, 0x8b, 0x70, 0x11, 0xab, 0x43, 0xea,
                0x96, 0x29, 0x31, 0xbe, 0x6d, 0xfc, 0x2e, 0xef, 0x99, 0x57, 0xb5, 0x53, 0x68, 0x9b,
                0x49, 0xf8, 0xdc, 0x91,
            ],
        );
        let mut proof_3: Vec<BytesN<32>> = Vec::new(&env);
        proof_3.push_back(root_2.clone());
        let computed_3 = client.compute_merkle_root(&3, &trace_2, &2, &proof_3);
        assert_eq!(computed_3, root_3);
    }

    #[test]
    fn publish_batch_and_verify_inclusion() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);

        let root = BytesN::from_array(
            &env,
            &[
                0xa5, 0xb9, 0x47, 0x06, 0xba, 0xa6, 0x8c, 0x77, 0x8b, 0x70, 0x11, 0xab, 0x43, 0xea,
                0x96, 0x29, 0x31, 0xbe, 0x6d, 0xfc, 0x2e, 0xef, 0x99, 0x57, 0xb5, 0x53, 0x68, 0x9b,
                0x49, 0xf8, 0xdc, 0x91,
            ],
        );
        let batch_id = client.publish_trace_batch(
            &agent,
            &root,
            &3,
            &String::from_str(&env, "ipfs://manifest-3"),
        );
        assert_eq!(batch_id, 0);
        assert_eq!(client.batch_count(), 1);
        assert_eq!(client.agent_batch_count(&agent), 1);
        assert_eq!(client.agent_batch_at(&agent, &0), 0);

        let batch = client.get_batch(&batch_id);
        assert_eq!(batch.agent, agent);
        assert_eq!(batch.root, root);
        assert_eq!(batch.leaf_count, 3);
        assert_eq!(batch.manifest_cid, String::from_str(&env, "ipfs://manifest-3"));

        let trace_2 = BytesN::from_array(
            &env,
            &[
                0x42, 0x4d, 0xa8, 0xb6, 0x08, 0x8f, 0xd9, 0xf1, 0xc2, 0xd0, 0xd1, 0x1c, 0x5a, 0x6c,
                0xd5, 0xd6, 0x1b, 0xf5, 0x31, 0x6d, 0xee, 0x8e, 0x61, 0xb3, 0xb0, 0xa3, 0x0f, 0x2a,
                0x01, 0xb8, 0xb4, 0xa1,
            ],
        );
        let sibling_root_2 = BytesN::from_array(
            &env,
            &[
                0xcd, 0x3a, 0x97, 0xb9, 0x45, 0x78, 0x20, 0x45, 0x0f, 0xf6, 0xd5, 0x77, 0xb6, 0xdd,
                0x8a, 0x13, 0xbe, 0xf9, 0x4a, 0xce, 0x08, 0xb8, 0xec, 0xba, 0xbe, 0x4e, 0x2c, 0xa4,
                0x74, 0x71, 0xe2, 0x29,
            ],
        );
        let mut proof: Vec<BytesN<32>> = Vec::new(&env);
        proof.push_back(sibling_root_2);

        // Valid inclusion check returns true
        assert!(client.verify_trace_inclusion(&batch_id, &trace_2, &2, &proof));

        // Wrong trace hash returns false
        let fake_trace = BytesN::from_array(&env, &[99u8; 32]);
        assert!(!client.verify_trace_inclusion(&batch_id, &fake_trace, &2, &proof));

        // Index out of bounds (index 3 with leaf_count 3) returns false
        assert!(!client.verify_trace_inclusion(&batch_id, &trace_2, &3, &proof));

        // Mutated proof element returns false
        let mut bad_proof: Vec<BytesN<32>> = Vec::new(&env);
        bad_proof.push_back(BytesN::from_array(&env, &[0u8; 32]));
        assert!(!client.verify_trace_inclusion(&batch_id, &trace_2, &2, &bad_proof));
    }

    #[test]
    fn merkle_second_preimage_resistance() {
        let env = Env::default();
        let trace = BytesN::from_array(&env, &[1u8; 32]);
        // Leaf hash is sha256(0x00 || trace)
        let mut leaf_preimage = Bytes::new(&env);
        leaf_preimage.push_back(MERKLE_LEAF_PREFIX);
        leaf_preimage.append(&trace.clone().into());
        let leaf_hash: BytesN<32> = env.crypto().sha256(&leaf_preimage).into();

        // Node hash is sha256(0x01 || left || right)
        let mut node_preimage = Bytes::new(&env);
        node_preimage.push_back(MERKLE_NODE_PREFIX);
        node_preimage.append(&trace.clone().into());
        node_preimage.append(&trace.into());
        let node_hash: BytesN<32> = env.crypto().sha256(&node_preimage).into();

        assert_ne!(leaf_hash, node_hash);
    }

    #[test]
    fn merkle_proof_length_bounded() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let root = BytesN::from_array(&env, &[1u8; 32]);
        let batch_id = client.publish_trace_batch(
            &agent,
            &root,
            &100,
            &String::from_str(&env, "ipfs://manifest"),
        );

        // Proof of length 13 (> MAX_PROOF_LEN = 12) must fail with InvalidProof
        let mut long_proof: Vec<BytesN<32>> = Vec::new(&env);
        for _ in 0..13 {
            long_proof.push_back(BytesN::from_array(&env, &[2u8; 32]));
        }
        assert_eq!(
            client.try_verify_trace_inclusion(&batch_id, &BytesN::from_array(&env, &[3u8; 32]), &0, &long_proof),
            Err(Ok(Error::InvalidProof))
        );
    }

    #[test]
    fn merkle_batch_size_bounded() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let root = BytesN::from_array(&env, &[1u8; 32]);

        // leaf_count = 0 fails
        assert_eq!(
            client.try_publish_trace_batch(
                &agent,
                &root,
                &0,
                &String::from_str(&env, "ipfs://manifest"),
            ),
            Err(Ok(Error::InvalidBatch))
        );

        // leaf_count = 4097 (> MAX_LEAVES = 4096) fails
        assert_eq!(
            client.try_publish_trace_batch(
                &agent,
                &root,
                &4097,
                &String::from_str(&env, "ipfs://manifest"),
            ),
            Err(Ok(Error::InvalidBatch))
        );

        // empty manifest_cid fails
        assert_eq!(
            client.try_publish_trace_batch(
                &agent,
                &root,
                &10,
                &String::from_str(&env, ""),
            ),
            Err(Ok(Error::InvalidBatch))
        );
    }

    #[test]
    fn unregistered_or_retired_agent_cannot_publish_batch() {
        let (env, client, _, agent) = setup();
        let root = BytesN::from_array(&env, &[1u8; 32]);

        // Unregistered agent fails
        assert_eq!(
            client.try_publish_trace_batch(
                &agent,
                &root,
                &5,
                &String::from_str(&env, "ipfs://manifest"),
            ),
            Err(Ok(Error::AgentNotRegistered))
        );

        // Register then retire
        client.register_agent(&agent);
        client.retire_agent(&agent);

        // Retired agent fails
        assert_eq!(
            client.try_publish_trace_batch(
                &agent,
                &root,
                &5,
                &String::from_str(&env, "ipfs://manifest"),
            ),
            Err(Ok(Error::AgentNotActive))
        );
    }
}
