import assert from "node:assert/strict";
import test from "node:test";
import { agentCredentials, StellarAdapter, type Deployments } from "./adapter.js";

const deployments: Deployments = {
  deployer: "GDEPLOYER",
  agent: "GAGENT",
  contracts: {
    usdc: "CUSDC",
    market_core: "CMARKETCORE",
    resolver: "CRESOLVER",
    treasury: "CTREASURY",
    reasoning_registry: "CREGISTRY",
  },
};

test("dry-run mode never requires an agent secret", () => {
  assert.doesNotThrow(() => {
    new StellarAdapter({
      mode: "dry-run",
      networkPassphrase: "Test SDF Network ; September 2015",
      rpcUrl: "https://soroban-testnet.stellar.org",
      deployments,
    });
  });
});

test("live mode without an agent secret throws immediately", () => {
  assert.throws(() => {
    new StellarAdapter({
      mode: "live",
      networkPassphrase: "Test SDF Network ; September 2015",
      rpcUrl: "https://soroban-testnet.stellar.org",
      deployments,
    });
  }, /agentSecret/);
});

test("adapter rejects unknown mode", () => {
  assert.throws(() => {
    new StellarAdapter({
      mode: "yolo" as unknown as "dry-run",
      networkPassphrase: "Test SDF Network ; September 2015",
      rpcUrl: "https://soroban-testnet.stellar.org",
      deployments,
    });
  }, /unknown mode/);
});

test("dry-run agentBuy describes the call without touching the network", async () => {
  const adapter = new StellarAdapter({
    mode: "dry-run",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
    deployments,
  });
  const result = await adapter.agentBuy({
    market_id: 0n,
    outcome: { tag: "Yes", values: undefined },
    collateral_in: 10_000_000n,
    min_shares_out: 0n,
  });
  assert.equal(result.dryRun, true);
  if (result.dryRun) {
    assert.equal(result.contractId, "CTREASURY");
    assert.equal(result.method, "agent_buy");
  }
});

test("dry-run agentBuy includes agent address", async () => {
  const adapter = new StellarAdapter({
    mode: "dry-run",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
    agentAddress: "GTRADER",
    deployments,
  });
  const result = await adapter.agentBuy({
    market_id: 0n,
    outcome: { tag: "Yes", values: undefined },
    collateral_in: 10_000_000n,
    min_shares_out: 1n,
  });
  assert.equal(result.dryRun, true);
  if (result.dryRun) {
    assert.equal((result.args as { agent: string }).agent, "GTRADER");
  }
  // Without a configured address, the constructor agent acts.
  const fallback = new StellarAdapter({
    mode: "dry-run",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
    deployments,
  });
  assert.equal(fallback.agent, "GAGENT");
});

test("market-maker uses maker key when configured", () => {
  const withAgents: Deployments = { ...deployments, agents: { maker: "GMAKER", trader: "GTRADER" } };
  assert.deepEqual(
    agentCredentials("maker", { MAKER_SECRET_KEY: "SMAKER", AGENT_SECRET_KEY: "SSHARED" }, withAgents),
    { agentSecret: "SMAKER", agentAddress: "GMAKER" },
  );
  assert.deepEqual(
    agentCredentials("trader", { MAKER_SECRET_KEY: "SMAKER", AGENT_SECRET_KEY: "SSHARED" }, withAgents),
    { agentSecret: "SSHARED", agentAddress: "GTRADER" },
  );
  // Pre-registry deployments fall back to the single shared agent.
  assert.deepEqual(agentCredentials("maker", { AGENT_SECRET_KEY: "SSHARED" }, deployments), {
    agentSecret: "SSHARED",
    agentAddress: "GAGENT",
  });
});

test("dry-run agentBuyV2 describes the bounded call without touching the network", async () => {
  const adapter = new StellarAdapter({
    mode: "dry-run",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
    deployments,
  });
  const result = await adapter.agentBuyV2({
    market_id: 0n,
    outcome: { tag: "Yes", values: undefined },
    collateral_in: 10_000_000n,
    min_shares_out: 9_500_000n,
    limit_price_bps: 6000,
    deadline: 1_000_060n,
  });
  assert.equal(result.dryRun, true);
  if (result.dryRun) {
    assert.equal(result.contractId, "CTREASURY");
    assert.equal(result.method, "agent_buy_v2");
  }
});

test("dry-run agentSellV2 describes the bounded call without touching the network", async () => {
  const adapter = new StellarAdapter({
    mode: "dry-run",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
    deployments,
  });
  const result = await adapter.agentSellV2({
    market_id: 0n,
    outcome: { tag: "No", values: undefined },
    collateral_out: 5_000_000n,
    max_shares_in: 6_000_000n,
    limit_price_bps: 4000,
    deadline: 1_000_060n,
  });
  assert.equal(result.dryRun, true);
  if (result.dryRun) {
    assert.equal(result.contractId, "CTREASURY");
    assert.equal(result.method, "agent_sell_v2");
  }
});

test("dry-run publishTrace passes u64::MAX sentinel exactly", async () => {
  const adapter = new StellarAdapter({
    mode: "dry-run",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
    deployments,
  });
  const U64_MAX = 18446744073709551615n;
  const result = await adapter.publishTrace({
    agent: "GAGENT",
    market_id: U64_MAX,
    action: { tag: "ProposeMarket", values: undefined },
    trace_hash: Buffer.alloc(32, 1),
    ipfs_cid: "ipfs://propose",
  });
  assert.equal(result.dryRun, true);
  if (result.dryRun) {
    assert.equal(result.contractId, "CREGISTRY");
    assert.equal(result.method, "publish_trace");
    assert.equal((result.args as any).market_id, U64_MAX);
    assert.deepEqual((result.args as any).action, { tag: "ProposeMarket", values: undefined });
  }
});

test("dry-run publishTraceBatch describes batch anchoring without network", async () => {
  const adapter = new StellarAdapter({
    mode: "dry-run",
    networkPassphrase: "Test SDF Network ; September 2015",
    rpcUrl: "https://soroban-testnet.stellar.org",
    deployments,
  });
  const root = Buffer.alloc(32, 2);
  const result = await adapter.publishTraceBatch({
    agent: "GAGENT",
    root,
    leaf_count: 50,
    manifest_cid: "ipfs://batch-manifest-50",
  });
  assert.equal(result.dryRun, true);
  if (result.dryRun) {
    assert.equal(result.contractId, "CREGISTRY");
    assert.equal(result.method, "publish_trace_batch");
    assert.deepEqual((result.args as any).root, root);
    assert.equal((result.args as any).leaf_count, 50);
    assert.equal((result.args as any).manifest_cid, "ipfs://batch-manifest-50");
  }
});
