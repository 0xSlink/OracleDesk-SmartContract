import type { RequestHandler } from "express";
import express, { type Express, type Request, type Response } from "express";
import { HTTPFacilitatorClient } from "@x402/core/server";
import { paymentMiddleware, x402ResourceServer } from "@x402/express";
import { ExactStellarScheme } from "@x402/stellar/exact/server";
import { ipfsUrl, verifyTraceHash, type TraceRecord } from "./trace-verification.js";

export type { TraceRecord };

export interface TraceRepository {
  get(traceId: string): Promise<TraceRecord | undefined>;
}

export interface TracePaymentNetwork {
  network: "stellar:testnet" | "stellar:pubnet";
  payTo: string;
  facilitatorUrl: string;
  facilitatorApiKey?: string;
}

export interface TraceApiOptions {
  repository: TraceRepository;
  ipfsGateway: string;
  price: string;
  networks: TracePaymentNetwork[];
  fetchImpl?: typeof fetch;
  verdictPolicy?: "refuse" | "label";
}

export class MapTraceRepository implements TraceRepository {
  public constructor(private readonly records: ReadonlyMap<string, TraceRecord>) {}

  public async get(traceId: string): Promise<TraceRecord | undefined> {
    return this.records.get(traceId);
  }
}

async function loadVerifiedTrace(
  repository: TraceRepository,
  gateway: string,
  traceId: string,
  fetchImpl: typeof fetch,
): Promise<{ record: TraceRecord; content: unknown } | undefined> {
  const record = await repository.get(traceId);
  if (!record) return undefined;

  const response = await fetchImpl(ipfsUrl(gateway, record.ipfsCid));
  if (!response.ok) {
    throw new Error(`IPFS gateway returned HTTP ${response.status}`);
  }

  const bytes = new Uint8Array(await response.arrayBuffer());
  if (!verifyTraceHash(bytes, record.traceHash)) {
    throw new Error(`Trace ${traceId} failed on-chain hash verification`);
  }

  try {
    return { record, content: JSON.parse(new TextDecoder().decode(bytes)) };
  } catch {
    throw new Error(`Trace ${traceId} is not valid JSON`);
  }
}

export function createVerdictPrePaymentMiddleware(options: TraceApiOptions): RequestHandler {
  return async (req: Request, res: Response, next) => {
    const verdictPolicy = options.verdictPolicy ?? "refuse";
    if (verdictPolicy === "refuse") {
      try {
        const traceId = Array.isArray(req.params.traceId)
          ? req.params.traceId[0]
          : req.params.traceId;
        const record = await options.repository.get(traceId);
        if (record && record.verdict && record.verdict.tag !== "Valid") {
          res.status(410).json({ error: "Trace verdict is not valid", verdict: record.verdict });
          return;
        }
      } catch {
        // Continue to payment handler on lookup error
      }
    }
    next();
  };
}

export function createTraceRouteHandler(
  options: TraceApiOptions,
  fetchImpl: typeof fetch = options.fetchImpl ?? fetch,
): RequestHandler {
  return async (req: Request, res: Response) => {
    try {
      const traceId = Array.isArray(req.params.traceId)
        ? req.params.traceId[0]
        : req.params.traceId;
      const result = await loadVerifiedTrace(
        options.repository,
        options.ipfsGateway,
        traceId,
        fetchImpl,
      );
      if (!result) {
        res.status(404).json({ error: "Trace not found" });
        return;
      }
      const responseData: Record<string, unknown> = {
        traceId: result.record.traceId,
        traceHash: result.record.traceHash,
        content: result.content,
      };
      if (options.verdictPolicy === "label" && result.record.verdict) {
        responseData.verdict = result.record.verdict;
      }
      res.json(responseData);
    } catch (error) {
      res.status(502).json({
        error: error instanceof Error ? error.message : "Trace retrieval failed",
      });
    }
  };
}

function createPaymentHandler(config: TracePaymentNetwork, options: TraceApiOptions): RequestHandler {
  const facilitator = new HTTPFacilitatorClient({
    url: config.facilitatorUrl,
    createAuthHeaders: config.facilitatorApiKey
      ? async () => {
          const headers = { Authorization: `Bearer ${config.facilitatorApiKey}` };
          return { verify: headers, settle: headers, supported: headers };
        }
      : undefined,
  });
  const x402Server = new x402ResourceServer(facilitator).register(
    config.network,
    new ExactStellarScheme(),
  );

  return paymentMiddleware(
    {
      "GET /traces/:traceId": {
        accepts: [{
          scheme: "exact",
          price: options.price,
          network: config.network,
          payTo: config.payTo,
        }],
        description: "Paid access to an OracleDesk reasoning trace",
      },
    },
    x402Server,
  );
}

export function createTraceApi(options: TraceApiOptions): Express {
  if (options.networks.length === 0) {
    throw new Error("At least one x402 Stellar payment network is required");
  }
  const fetchImpl = options.fetchImpl ?? fetch;
  const app = express();

  const verdictPolicy = options.verdictPolicy ?? "refuse";
  if (verdictPolicy === "refuse") {
    app.get("/traces/:traceId", createVerdictPrePaymentMiddleware(options));
  }

  for (const network of options.networks) {
    app.use(createPaymentHandler(network, options));
  }

  app.get("/traces/:traceId", createTraceRouteHandler(options, fetchImpl));

  return app;
}
