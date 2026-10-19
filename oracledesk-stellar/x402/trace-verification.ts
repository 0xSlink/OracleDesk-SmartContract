import { createHash, createPublicKey, verify } from "node:crypto";

export type TraceVerdict =
  | { tag: "Valid"; values?: void }
  | { tag: "HashMismatch"; values?: void }
  | { tag: "Flagged"; values: [string] | readonly [string] }
  | { tag: "AgentRevokedAfter"; values: [bigint] | readonly [bigint] };

export interface TraceRecord {
  traceId: string;
  ipfsCid: string;
  traceHash: string;
  action?: string;
  verdict?: TraceVerdict;
}

export function sha256Hex(content: Uint8Array): string {
  return createHash("sha256").update(content).digest("hex");
}

export function verifyTraceHash(content: Uint8Array, expectedHash: string): boolean {
  const normalized = expectedHash.toLowerCase().replace(/^0x/, "");
  return /^[0-9a-f]{64}$/.test(normalized) && sha256Hex(content) === normalized;
}

export function ipfsUrl(gateway: string, cid: string): string {
  const base = gateway.endsWith("/") ? gateway.slice(0, -1) : gateway;
  return `${base}/${encodeURIComponent(cid)}`;
}

// ─── Merkle batch inclusion (#54) ──────────────────────────────────────────

export const MAX_BATCH_LEAVES = 4096;
export const MAX_BATCH_PROOF_LEN = 12;

export function hashMerkleLeaf(traceHashHex: string): string {
  const norm = traceHashHex.toLowerCase().replace(/^0x/, "");
  return createHash("sha256")
    .update(Buffer.from([0x00]))
    .update(Buffer.from(norm, "hex"))
    .digest("hex");
}

export function hashMerkleNode(leftHex: string, rightHex: string): string {
  const left = leftHex.toLowerCase().replace(/^0x/, "");
  const right = rightHex.toLowerCase().replace(/^0x/, "");
  return createHash("sha256")
    .update(Buffer.from([0x01]))
    .update(Buffer.from(left, "hex"))
    .update(Buffer.from(right, "hex"))
    .digest("hex");
}

export function verifyBatchInclusion(
  rootHex: string,
  leafCount: number,
  traceHashHex: string,
  index: number,
  proofHex: string[],
): boolean {
  if (leafCount <= 0 || leafCount > MAX_BATCH_LEAVES) return false;
  if (index < 0 || index >= leafCount) return false;
  if (proofHex.length > MAX_BATCH_PROOF_LEN) return false;

  let current = hashMerkleLeaf(traceHashHex);
  let currIdx = index;
  let currLen = leafCount;
  let proofIdx = 0;

  while (currLen > 1) {
    if (currLen % 2 === 1 && currIdx === currLen - 1) {
      // Lone odd node promoted without sibling
    } else {
      if (proofIdx >= proofHex.length) return false;
      const sibling = proofHex[proofIdx++];
      if (currIdx % 2 === 0) {
        current = hashMerkleNode(current, sibling);
      } else {
        current = hashMerkleNode(sibling, current);
      }
    }

    currIdx = Math.floor(currIdx / 2);
    currLen = Math.floor((currLen + 1) / 2);
  }

  const normRoot = rootHex.toLowerCase().replace(/^0x/, "");
  return proofIdx === proofHex.length && current.toLowerCase() === normRoot;
}

// ─── Hash-chained traces (#52) ───────────────────────────────────────────────

/** One link of an agent's on-chain trace log, as read from the registry. */
export interface ChainEntry {
  seq: bigint;
  traceHash: string;
  prevTraceHash: string;
}

export type ChainVerdict =
  | { ok: true }
  | { ok: false; reason: "gap" | "broken-link" | "bad-genesis"; atSeq: bigint };

const GENESIS_HASH = "0".repeat(64);

function normalizeHash(hash: string): string {
  return hash.toLowerCase().replace(/^0x/, "");
}

/**
 * Walks a contiguous range of an agent's chain and reports the first
 * problem: a missing sequence number (`gap`), a link whose `prevTraceHash`
 * isn't the previous entry's hash (`broken-link`), or — when the range is
 * meant to start at the beginning — a first entry that isn't seq 1 on the
 * all-zero genesis hash (`bad-genesis`). Entries must be sorted by `seq`.
 */
export function verifyChain(entries: ChainEntry[], fromGenesis: boolean): ChainVerdict {
  if (entries.length === 0) return { ok: true };

  const first = entries[0];
  if (fromGenesis && (first.seq !== 1n || normalizeHash(first.prevTraceHash) !== GENESIS_HASH)) {
    return { ok: false, reason: "bad-genesis", atSeq: first.seq };
  }

  for (let i = 1; i < entries.length; i += 1) {
    const prev = entries[i - 1];
    const cur = entries[i];
    if (cur.seq !== prev.seq + 1n) {
      return { ok: false, reason: "gap", atSeq: prev.seq + 1n };
    }
    if (normalizeHash(cur.prevTraceHash) !== normalizeHash(prev.traceHash)) {
      return { ok: false, reason: "broken-link", atSeq: cur.seq };
    }
  }
  return { ok: true };
}

// ─── Ed25519 trace attestations (#53) ───────────────────────────────────────

/**
 * Verifies an Ed25519 signature over a message using Node's standard crypto.
 * Can be run completely offline with no network or registry access.
 */
export function verifyEd25519Signature(
  publicKey: Uint8Array,
  message: Uint8Array,
  signature: Uint8Array,
): boolean {
  if (publicKey.length !== 32 || signature.length !== 64) return false;
  try {
    const spkiPrefix = Buffer.from("302a300506032b6570032100", "hex");
    const der = Buffer.concat([spkiPrefix, Buffer.from(publicKey)]);
    const key = createPublicKey({ key: der, format: "der", type: "spki" });
    return verify(null, Buffer.from(message), key, Buffer.from(signature));
  } catch {
    return false;
  }
}

