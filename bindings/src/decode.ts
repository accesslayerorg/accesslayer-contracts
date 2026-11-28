/**
 * Shared runtime helpers for decoding raw Soroban contract events.
 *
 * The per-contract decoders in `events.ts` are generated from the contracts'
 * event definitions; this module holds the part that never changes: turning
 * RPC/XDR event shapes into plain JavaScript values.
 */
import { scValToNative, xdr } from "@stellar/stellar-sdk";

/**
 * The shapes events arrive in depending on where they are read from:
 * - `rpc.Server.getEvents()` -> `{ topics: xdr.ScVal[], value: xdr.ScVal }`
 * - `AssembledTransaction` result / `sentEvents` -> `{ topics, data }`
 * - already decoded test fixtures -> plain values
 */
export interface RawContractEvent {
  topics?: readonly unknown[];
  data?: unknown;
  /** `ContractEvent.value` as returned by Soroban RPC. */
  value?: unknown;
}

export interface DecodedContractEvent {
  /** Event name (topic index 0) as a string, e.g. `"buy"`. */
  name: string;
  /** Decoded topics, with the event name at index 0. */
  topics: readonly unknown[];
  /** Decoded event payload. Struct payloads become plain objects. */
  data: unknown;
}

function isScVal(value: unknown): value is xdr.ScVal {
  return value instanceof xdr.ScVal;
}

function toNative(value: unknown): unknown {
  if (!isScVal(value)) return value;
  try {
    return scValToNative(value);
  } catch {
    return value;
  }
}

/**
 * Decodes the topics and payload of a contract event.
 *
 * Returns `null` when the event has no topics, which is the only case where the
 * event name cannot be recovered.
 */
export function decodeEvent(event: RawContractEvent): DecodedContractEvent | null {
  const rawTopics = event.topics ?? [];
  if (rawTopics.length === 0) return null;

  const topics = rawTopics.map(toNative);
  const name = String(topics[0]);
  const data = toNative(event.data ?? event.value);

  return { name, topics, data };
}
