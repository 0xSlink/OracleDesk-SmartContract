import assert from "node:assert/strict";
import test from "node:test";
import { RegistryTraceRepository, type RegistryTraceReader } from "./registry-trace-repository.js";

function fakeReader(byId: Map<bigint, { trace_hash: string; ipfs_cid: string }>): RegistryTraceReader {
  return {
    async get_trace({ trace_id }) {
      const record = byId.get(trace_id);
      if (!record) throw new Error("Error(Contract, #5)"); // TraceNotFound
      return {
        result: {
          market_id: 0n,
          agent: "GAGENT",
          action: "create_market",
          trace_hash: Buffer.from(record.trace_hash, "hex"),
          ipfs_cid: record.ipfs_cid,
          published_at: 0n,
        },
      };
    },
  };
}

test("returns a TraceRecord translated from the on-chain trace", async () => {
  const hash = "ab".repeat(32);
  const repo = new RegistryTraceRepository(
    fakeReader(new Map([[0n, { trace_hash: hash, ipfs_cid: "ipfs://x" }]])),
  );
  const record = await repo.get("0");
  assert.deepEqual(record, {
    traceId: "0",
    ipfsCid: "ipfs://x",
    traceHash: hash,
    action: "create_market",
    verdict: undefined,
  });
});

test("maps verdict from reader", async () => {
  const hash = "ab".repeat(32);
  const reader: RegistryTraceReader = {
    async get_trace() {
      return {
        result: {
          market_id: 0n,
          agent: "GAGENT",
          action: "create_market",
          trace_hash: Buffer.from(hash, "hex"),
          ipfs_cid: "ipfs://x",
          published_at: 0n,
        },
      };
    },
    async trace_verdict() {
      return {
        result: { tag: "Flagged", values: ["fraud"] },
      };
    },
  };
  const repo = new RegistryTraceRepository(reader);
  const record = await repo.get("0");
  assert.deepEqual(record, {
    traceId: "0",
    ipfsCid: "ipfs://x",
    traceHash: hash,
    action: "create_market",
    verdict: { tag: "Flagged", values: ["fraud"] },
  });
});

test("normalizes tagged action to string", async () => {
  const hash = "ab".repeat(32);
  const reader: RegistryTraceReader = {
    async get_trace() {
      return {
        result: {
          market_id: 0n,
          agent: "GAGENT",
          action: { tag: "Buy", values: undefined },
          trace_hash: Buffer.from(hash, "hex"),
          ipfs_cid: "ipfs://x",
          published_at: 0n,
        },
      };
    },
  };
  const repo = new RegistryTraceRepository(reader);
  const record = await repo.get("0");
  assert.equal(record?.action, "buy");
});

test("returns undefined for a trace id the registry does not know", async () => {
  const repo = new RegistryTraceRepository(fakeReader(new Map()));
  assert.equal(await repo.get("999"), undefined);
});

test("returns undefined rather than throwing for a non-numeric trace id", async () => {
  const repo = new RegistryTraceRepository(fakeReader(new Map()));
  assert.equal(await repo.get("not-a-number"), undefined);
});
