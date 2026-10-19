#![no_std]

// The test module builds over-length `action`/`ipfs_cid` strings with
// `str::repeat`, which lives in `alloc`/`std`; the crate itself stays
// `no_std`.
#[cfg(test)]
extern crate std;

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, Bytes, BytesN,
    Env, String, Vec,
};

/// Domain tag for commit-reveal commitments (#51). Must match
/// `agents/core/trace.ts` byte-for-byte: `sha256(TAG || trace_hash || salt)`.
pub const COMMIT_DOMAIN_TAG: &[u8] = b"oracledesk.trace.commit.v1";

const BUMP_THRESHOLD: u32 = 30 * 17_280;
const BUMP_TO: u32 = 60 * 17_280;

/// Longest accepted `action` string, in bytes (#57). Bounding every trace's
/// variable-length fields is what makes the paginated batch reads below
/// provably safe: see [`MAX_PAGE_SIZE`].
pub const MAX_ACTION_LEN: u32 = 64;
/// Longest accepted IPFS CID/URI string, in bytes (#57).
pub const MAX_CID_LEN: u32 = 128;
/// Largest page a batch read returns (#57).
///
/// Safety bound: a `Trace`'s worst-case XDR is
/// `market_id` 8 + `agent` address 56 + `action` (4 + 64) + `trace_hash` 32
/// + `ipfs_cid` (4 + 128) + `published_at` 8 ≈ 304 bytes. Wrapping each entry
/// in `TracePageEntry` adds 8 bytes for `trace_id` and a small vector overhead,
/// so one entry is under 340 bytes. A page of 25 is therefore under ~8.5 KiB
/// of ledger reads, plus at most 25 storage keys of ~50 bytes each — an order
/// of magnitude below Soroban's per-transaction read limits even if every
/// trace is maximal length. (Before this bound existed, `action`/`ipfs_cid`
/// were unbounded, so no page size could have been proven safe.)
pub const MAX_PAGE_SIZE: u32 = 25;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trace {
    pub market_id: u64,
    pub agent: Address,
    pub action: String,
    pub trace_hash: BytesN<32>,
    pub ipfs_cid: String,
    pub published_at: u64,
}

/// One `(trace_id, Trace)` pair in a [`TracePage`].
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TracePageEntry {
    pub trace_id: u64,
    pub trace: Trace,
}

/// A page of traces returned by one simulation, plus the cursor to fetch the
/// next page (#57). `next == total` means the caller has seen everything.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TracePage {
    pub entries: Vec<TracePageEntry>,
    /// Index of the first entry the *next* page would return.
    pub next: u64,
    /// Total entries in this index at read time.
    pub total: u64,
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
    /// Number of traces published by an agent, in publish order (#57).
    AgentTraceCount(Address),
    /// `agent, index -> trace_id`, a secondary index parallel to
    /// `MarketTrace` so "everything agent X published" is one page (#57).
    AgentTraceAt(Address, u64),
    /// Number of traces for a `(market, action)` pair (#57).
    MarketActionTraceCount(u64, String),
    /// `market, action, index -> trace_id`, so "every Buy decision on market
    /// 7" is one page instead of scanning the market's whole index (#57).
    MarketActionTraceAt(u64, String, u64),
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

/// Base of the reasoning-registry error-code range, disjoint from
/// market-core (`1..=999`), resolver (`1001..=1999`) and treasury
/// (`2001..=2999`) (see `docs/adr/0006-cross-contract-errors.md`).
pub const REGISTRY_ERROR_BASE: u32 = 3_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Error {
    NotInitialized = 3001,
    Unauthorized = 3002,
    AgentAlreadyRegistered = 3003,
    AgentNotRegistered = 3004,
    TraceNotFound = 3005,
    EmptyAction = 3006,
    EmptyCid = 3007,
    /// `expected_seq` / `prev_trace_hash` don't extend the agent's chain head (#52).
    ChainHeadMismatch = 3008,
    /// Agent already has a chain, so unchained `publish_trace` is refused (#52).
    ChainedAgentMustChain = 3009,
    /// Trace already backed an action (#50).
    TraceAlreadyConsumed = 3010,
    /// `action` is longer than [`MAX_ACTION_LEN`], which would make paginated
    /// reads unbounded (#57).
    ActionTooLong = 3011,
    /// `ipfs_cid` is longer than [`MAX_CID_LEN`] (#57).
    CidTooLong = 3012,
    /// `limit` exceeded [`MAX_PAGE_SIZE`] (#57).
    PageTooLarge = 3013,
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
    pub action: String,
    pub trace_hash: BytesN<32>,
    pub ipfs_cid: String,
    pub published_at: u64,
}

#[contract]
pub struct ReasoningRegistry;

#[contractimpl]
impl ReasoningRegistry {
    pub fn __constructor(env: Env, admin: Address) {
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextId, &0u64);
    }

    pub fn register_agent(env: Env, agent: Address) -> Result<(), Error> {
        Self::require_admin(&env)?;
        let key = DataKey::Agent(agent.clone());
        if env.storage().persistent().has(&key) {
            return Err(Error::AgentAlreadyRegistered);
        }
        env.storage().persistent().set(&key, &true);
        env.storage()
            .persistent()
            .extend_ttl(&key, BUMP_THRESHOLD, BUMP_TO);
        AgentRegistered { agent }.publish(&env);
        Ok(())
    }

    pub fn unregister_agent(env: Env, agent: Address) -> Result<(), Error> {
        Self::require_admin(&env)?;
        let key = DataKey::Agent(agent);
        if !env.storage().persistent().has(&key) {
            return Err(Error::AgentNotRegistered);
        }
        env.storage().persistent().remove(&key);
        Ok(())
    }

    pub fn is_registered(env: Env, agent: Address) -> bool {
        env.storage().persistent().has(&DataKey::Agent(agent))
    }

    pub fn publish_trace(
        env: Env,
        agent: Address,
        market_id: u64,
        action: String,
        trace_hash: BytesN<32>,
        ipfs_cid: String,
    ) -> Result<u64, Error> {
        Self::check_publish(&env, &agent, &action, &ipfs_cid)?;
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
        action: String,
        trace_hash: BytesN<32>,
        ipfs_cid: String,
        expected_seq: u64,
        prev_trace_hash: BytesN<32>,
    ) -> Result<u64, Error> {
        Self::check_publish(&env, &agent, &action, &ipfs_cid)?;
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

    pub fn verify_trace(env: Env, trace_id: u64, received_hash: BytesN<32>) -> Result<bool, Error> {
        let trace = Self::get_trace(env, trace_id)?;
        Ok(trace.trace_hash == received_hash)
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

    /// Number of traces published by `agent` (#57).
    pub fn agent_trace_count(env: Env, agent: Address) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::AgentTraceCount(agent))
            .unwrap_or(0)
    }

    /// The `index`-th (0-based, publish order) trace id for `agent` (#57).
    pub fn agent_trace_at(env: Env, agent: Address, index: u64) -> Result<u64, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::AgentTraceAt(agent, index))
            .ok_or(Error::TraceNotFound)
    }

    /// Number of traces for the `(market_id, action)` pair (#57).
    pub fn market_action_trace_count(env: Env, market_id: u64, action: String) -> u64 {
        env.storage()
            .persistent()
            .get(&DataKey::MarketActionTraceCount(market_id, action))
            .unwrap_or(0)
    }

    /// The `index`-th (0-based, publish order) trace id for
    /// `(market_id, action)` (#57).
    pub fn market_action_trace_at(
        env: Env,
        market_id: u64,
        action: String,
        index: u64,
    ) -> Result<u64, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::MarketActionTraceAt(market_id, action, index))
            .ok_or(Error::TraceNotFound)
    }

    /// A page of every trace for `market_id`, starting at `start` (#57).
    /// Replaces the `market_trace_count` + `market_trace_at` + `get_trace`
    /// loop (101 RPC simulations for 50 traces) with one simulation.
    pub fn traces_by_market(
        env: Env,
        market_id: u64,
        start: u64,
        limit: u32,
    ) -> Result<TracePage, Error> {
        let total = Self::market_trace_count(env.clone(), market_id);
        Self::trace_page(&env, total, start, limit, |index| {
            env.storage()
                .persistent()
                .get(&DataKey::MarketTrace(market_id, index))
                .ok_or(Error::TraceNotFound)
        })
    }

    /// A page of every trace `agent` published, in publish order (#57).
    pub fn traces_by_agent(
        env: Env,
        agent: Address,
        start: u64,
        limit: u32,
    ) -> Result<TracePage, Error> {
        let total = Self::agent_trace_count(env.clone(), agent.clone());
        Self::trace_page(&env, total, start, limit, |index| {
            env.storage()
                .persistent()
                .get(&DataKey::AgentTraceAt(agent.clone(), index))
                .ok_or(Error::TraceNotFound)
        })
    }

    /// A page of every trace for `(market_id, action)`, in publish order
    /// (#57) — e.g. every `Buy` decision on market 7.
    pub fn traces_by_market_action(
        env: Env,
        market_id: u64,
        action: String,
        start: u64,
        limit: u32,
    ) -> Result<TracePage, Error> {
        let total = Self::market_action_trace_count(env.clone(), market_id, action.clone());
        Self::trace_page(&env, total, start, limit, |index| {
            env.storage()
                .persistent()
                .get(&DataKey::MarketActionTraceAt(
                    market_id,
                    action.clone(),
                    index,
                ))
                .ok_or(Error::TraceNotFound)
        })
    }
}

impl ReasoningRegistry {
    fn check_publish(
        env: &Env,
        agent: &Address,
        action: &String,
        ipfs_cid: &String,
    ) -> Result<(), Error> {
        agent.require_auth();
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Agent(agent.clone()))
        {
            return Err(Error::AgentNotRegistered);
        }
        if action.is_empty() {
            return Err(Error::EmptyAction);
        }
        if action.len() > MAX_ACTION_LEN {
            return Err(Error::ActionTooLong);
        }
        if ipfs_cid.is_empty() {
            return Err(Error::EmptyCid);
        }
        if ipfs_cid.len() > MAX_CID_LEN {
            return Err(Error::CidTooLong);
        }
        Ok(())
    }

    /// Shared body of every publish path: allocates the id, stores the trace,
    /// its ledger and the per-market index, and emits `TracePublished`.
    fn write_trace(
        env: &Env,
        agent: Address,
        market_id: u64,
        action: String,
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

        // Secondary index by agent (#57).
        let agent_count_key = DataKey::AgentTraceCount(agent.clone());
        let agent_index: u64 = env.storage().persistent().get(&agent_count_key).unwrap_or(0);
        Self::put(env, &DataKey::AgentTraceAt(agent.clone(), agent_index), &id);
        Self::put(env, &agent_count_key, &(agent_index + 1));

        // Secondary index by (market, action) (#57).
        let action_count_key = DataKey::MarketActionTraceCount(market_id, action.clone());
        let action_index: u64 = env
            .storage()
            .persistent()
            .get(&action_count_key)
            .unwrap_or(0);
        Self::put(
            env,
            &DataKey::MarketActionTraceAt(market_id, action.clone(), action_index),
            &id,
        );
        Self::put(env, &action_count_key, &(action_index + 1));

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

    /// Shared body of the three paginated readers (#57): validates the page
    /// size, resolves up to `limit` trace ids through `id_at`, and loads each
    /// `Trace` into one return value. `total` is read once by the caller so a
    /// page can never straddle an inconsistent index length.
    fn trace_page(
        env: &Env,
        total: u64,
        start: u64,
        limit: u32,
        id_at: impl Fn(u64) -> Result<u64, Error>,
    ) -> Result<TracePage, Error> {
        if limit == 0 || limit > MAX_PAGE_SIZE {
            return Err(Error::PageTooLarge);
        }
        let mut entries = Vec::new(env);
        let end = core::cmp::min(start.saturating_add(limit as u64), total);
        let mut index = start;
        while index < end {
            let trace_id = id_at(index)?;
            let trace = Self::get_trace(env.clone(), trace_id)?;
            entries.push_back(TracePageEntry { trace_id, trace });
            index += 1;
        }
        Ok(TracePage {
            entries,
            next: end,
            total,
        })
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

    fn setup() -> (Env, ReasoningRegistryClient<'static>, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let agent = Address::generate(&env);
        let contract_id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin),
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
            &String::from_str(&env, "trade"),
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
        assert_eq!(trace.action, String::from_str(&env, "trade"));
        assert_eq!(trace.ipfs_cid, String::from_str(&env, "ipfs://trace-42"));
        assert!(client.is_registered(&agent));

        client.unregister_agent(&agent);
        assert!(!client.is_registered(&agent));
        let _ = admin;
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #3004)")]
    fn unregistered_agent_cannot_publish() {
        let (env, client, _, agent) = setup();
        client.publish_trace(
            &agent,
            &1,
            &String::from_str(&env, "pass"),
            &BytesN::from_array(&env, &[1u8; 32]),
            &String::from_str(&env, "ipfs://unauthorized"),
        );
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #3005)")]
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
            &String::from_str(&env, "create_market"),
            &BytesN::from_array(&env, &[1u8; 32]),
            &String::from_str(&env, "ipfs://t0"),
        );
        let t1 = client.publish_trace(
            &agent,
            &7,
            &String::from_str(&env, "trade"),
            &BytesN::from_array(&env, &[2u8; 32]),
            &String::from_str(&env, "ipfs://t1"),
        );
        // A trace on a different market must not appear in market 7's index.
        client.publish_trace(
            &agent,
            &8,
            &String::from_str(&env, "trade"),
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
                        String::from_str(&env, "trade"),
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
                &String::from_str(&env, "trade"),
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
                &String::from_str(env, "buy"),
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
            &String::from_str(&env, "buy"),
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
            &String::from_str(&env, "buy"),
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
        let id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin),
        );
        let client = ReasoningRegistryClient::new(&env, &id);
        env.mock_all_auths();
        client.register_agent(&agent);

        let action = String::from_str(&env, "buy");
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
            &String::from_str(&env, "buy"),
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
            &String::from_str(&env, "buy"),
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
        let id = env.register(
            ReasoningRegistry,
            ReasoningRegistryArgs::__constructor(&admin),
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

    // ── #57 secondary indexes and paginated batch reads ───────────────────

    #[test]
    fn traces_are_indexed_by_agent_and_market_action() {
        let (env, client, _, agent_a) = setup();
        let agent_b = Address::generate(&env);
        client.register_agent(&agent_a);
        client.register_agent(&agent_b);

        let buy = String::from_str(&env, "buy");
        let sell = String::from_str(&env, "sell");
        let t0 = client.publish_trace(
            &agent_a,
            &7,
            &buy,
            &BytesN::from_array(&env, &[1u8; 32]),
            &String::from_str(&env, "ipfs://a0"),
        );
        let t1 = client.publish_trace(
            &agent_b,
            &7,
            &buy,
            &BytesN::from_array(&env, &[2u8; 32]),
            &String::from_str(&env, "ipfs://b0"),
        );
        let t2 = client.publish_trace(
            &agent_a,
            &7,
            &sell,
            &BytesN::from_array(&env, &[3u8; 32]),
            &String::from_str(&env, "ipfs://a1"),
        );
        // A trace on a different market must not appear in market 7's
        // (market, action) index.
        client.publish_trace(
            &agent_a,
            &8,
            &buy,
            &BytesN::from_array(&env, &[4u8; 32]),
            &String::from_str(&env, "ipfs://a2"),
        );

        assert_eq!(client.agent_trace_count(&agent_a), 3);
        assert_eq!(client.agent_trace_at(&agent_a, &0), t0);
        assert_eq!(client.agent_trace_at(&agent_a, &2), t2);
        assert_eq!(client.agent_trace_count(&agent_b), 1);
        assert_eq!(client.agent_trace_at(&agent_b, &0), t1);

        assert_eq!(client.market_action_trace_count(&7, &buy), 2);
        assert_eq!(client.market_action_trace_at(&7, &buy, &0), t0);
        assert_eq!(client.market_action_trace_at(&7, &buy, &1), t1);
        assert_eq!(client.market_action_trace_count(&7, &sell), 1);
        assert_eq!(client.market_action_trace_count(&8, &buy), 1);
    }

    #[test]
    fn batch_reads_return_pages_with_cursor_and_total() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let buy = String::from_str(&env, "buy");
        for i in 0..5u8 {
            client.publish_trace(
                &agent,
                &7,
                &buy,
                &BytesN::from_array(&env, &[i; 32]),
                &String::from_str(&env, "ipfs://page"),
            );
        }

        let first = client.traces_by_market(&7, &0, &2);
        assert_eq!(first.total, 5);
        assert_eq!(first.next, 2);
        assert_eq!(first.entries.len(), 2);
        assert_eq!(first.entries.get(0).unwrap().trace_id, 0);
        assert_eq!(first.entries.get(1).unwrap().trace_id, 1);

        let second = client.traces_by_agent(&agent, &2, &2);
        assert_eq!(second.total, 5);
        assert_eq!(second.next, 4);
        assert_eq!(second.entries.get(0).unwrap().trace_id, 2);

        let last = client.traces_by_market_action(&7, &buy, &4, &2);
        assert_eq!(last.total, 5);
        assert_eq!(last.next, 5);
        assert_eq!(last.entries.len(), 1);
        assert_eq!(last.entries.get(0).unwrap().trace_id, 4);
        // The loaded Trace is the real record, not just an id.
        assert_eq!(last.entries.get(0).unwrap().trace.market_id, 7);

        // A cursor past the end is an empty page, not an error.
        let empty = client.traces_by_market(&7, &99, &5);
        assert_eq!(empty.entries.len(), 0);
        assert_eq!(empty.next, 99);
    }

    #[test]
    fn page_size_is_bounded() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        assert_eq!(
            client.try_traces_by_agent(&agent, &0, &(MAX_PAGE_SIZE + 1)),
            Err(Ok(Error::PageTooLarge))
        );
        assert_eq!(
            client.try_traces_by_market(&7, &0, &0),
            Err(Ok(Error::PageTooLarge))
        );
    }

    #[test]
    fn overlong_action_or_cid_is_rejected() {
        let (env, client, _, agent) = setup();
        client.register_agent(&agent);
        let long_action = String::from_str(&env, &"a".repeat(MAX_ACTION_LEN as usize + 1));
        assert_eq!(
            client.try_publish_trace(
                &agent,
                &1,
                &long_action,
                &BytesN::from_array(&env, &[1u8; 32]),
                &String::from_str(&env, "ipfs://ok"),
            ),
            Err(Ok(Error::ActionTooLong))
        );
        let long_cid = String::from_str(&env, &"c".repeat(MAX_CID_LEN as usize + 1));
        assert_eq!(
            client.try_publish_trace(
                &agent,
                &1,
                &String::from_str(&env, "buy"),
                &BytesN::from_array(&env, &[1u8; 32]),
                &long_cid,
            ),
            Err(Ok(Error::CidTooLong))
        );
    }
}
