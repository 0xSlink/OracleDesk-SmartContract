import assert from "node:assert/strict";
import test from "node:test";
import express from "express";
import {
  createVerdictPrePaymentMiddleware,
  createTraceRouteHandler,
  type TraceApiOptions,
  type TraceRepository,
  type TraceRecord,
} from "./trace-api.js";
import { sha256Hex } from "./trace-verification.js";

function createStubRepo(records: Map<string, TraceRecord>): TraceRepository {
  return {
    async get(id: string) {
      return records.get(id);
    },
  };
}

test("refuse policy returns 410 before payment for flagged trace", async () => {
  const content = JSON.stringify({ reason: "test content" });
  const contentBytes = new TextEncoder().encode(content);
  const hash = sha256Hex(contentBytes);
  const repo = createStubRepo(
    new Map([
      [
        "1",
        {
          traceId: "1",
          ipfsCid: "cid-1",
          traceHash: hash,
          verdict: { tag: "Flagged", values: ["fraud"] },
        },
      ],
    ]),
  );

  const options: TraceApiOptions = {
    repository: repo,
    ipfsGateway: "https://ipfs.io/ipfs",
    price: "0.01",
    networks: [],
    verdictPolicy: "refuse",
  };

  const app = express();
  app.get("/traces/:traceId", createVerdictPrePaymentMiddleware(options));
  app.get("/traces/:traceId", (_req, res) => {
    res.status(200).json({ ok: true });
  });

  const server = app.listen(0);
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;

  try {
    const res = await fetch(`http://127.0.0.1:${port}/traces/1`);
    assert.equal(res.status, 410);
    const body = (await res.json()) as { error: string; verdict: unknown };
    assert.deepEqual(body.verdict, { tag: "Flagged", values: ["fraud"] });
  } finally {
    server.close();
  }
});

test("label policy includes verdict", async () => {
  const content = JSON.stringify({ reason: "test content" });
  const contentBytes = new TextEncoder().encode(content);
  const hash = sha256Hex(contentBytes);
  const repo = createStubRepo(
    new Map([
      [
        "1",
        {
          traceId: "1",
          ipfsCid: "cid-1",
          traceHash: hash,
          verdict: { tag: "Flagged", values: ["fraud"] },
        },
      ],
    ]),
  );

  const fakeFetch = async () =>
    new Response(contentBytes, {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });

  const options: TraceApiOptions = {
    repository: repo,
    ipfsGateway: "https://ipfs.io/ipfs",
    price: "0.01",
    networks: [],
    verdictPolicy: "label",
    fetchImpl: fakeFetch as unknown as typeof fetch,
  };

  const app = express();
  app.get("/traces/:traceId", createVerdictPrePaymentMiddleware(options));
  app.get(
    "/traces/:traceId",
    createTraceRouteHandler(options, fakeFetch as unknown as typeof fetch),
  );

  const server = app.listen(0);
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;

  try {
    const res = await fetch(`http://127.0.0.1:${port}/traces/1`);
    assert.equal(res.status, 200);
    const body = (await res.json()) as {
      traceId: string;
      traceHash: string;
      content: unknown;
      verdict: unknown;
    };
    assert.equal(body.traceId, "1");
    assert.deepEqual(body.verdict, { tag: "Flagged", values: ["fraud"] });
    assert.deepEqual(body.content, { reason: "test content" });
  } finally {
    server.close();
  }
});
