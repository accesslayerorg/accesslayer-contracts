// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: key-liquidity-depth
// WASM SHA-256: ea0a4d2125f12902cd009c7ed53a7fa21a7124260d90db1a76aec24a379fd657
// Regenerate with: ./scripts/generate-bindings.sh
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




/**
 * Storage keys for persistent state.
 */
export type DataKey = {tag: "PriceLevels", values: readonly [string, OrderSide]} | {tag: "LevelVolume", values: readonly [string, OrderSide, i128]};

/**
 * Side of an order in the order book.
 */
export enum OrderSide {
  Buy = 0,
  Sell = 1,
}


/**
 * A single price level in the order book depth with cumulative volume.
 */
export interface DepthLevel {
  /**
 * Cumulative volume up to and including this price level.
 */
cumulative_volume: i128;
  /**
 * Price per unit at this level.
 */
price: i128;
  /**
 * Volume available at this specific price level.
 */
volume: i128;
}


/**
 * Event payload emitted when order book depth changes.
 * 
 * Event shape:
 * - topics: `(DEPTH_UPDATED_EVENT_NAME, key_id)`
 * - data: `DepthUpdated`
 */
export interface DepthUpdated {
  /**
 * Order amount added or cancelled.
 */
amount: i128;
  /**
 * Creator key whose order depth changed.
 */
key_id: string;
  /**
 * Price level affected.
 */
price: i128;
  /**
 * Order side (Buy or Sell).
 */
side: OrderSide;
  /**
 * Remaining total volume at this price level after the update.
 */
volume: i128;
}


/**
 * Full liquidity depth summary containing top bid and ask levels.
 */
export interface LiquidityDepth {
  /**
 * Ask levels sorted in ascending order (lowest price first).
 */
asks: Array<DepthLevel>;
  /**
 * Bid levels sorted in descending order (highest price first).
 */
bids: Array<DepthLevel>;
}


export interface LiquidityQuote {
  amount: i128;
  expected_price: i128;
  price_impact_bps: u32;
}

/**
 * Errors returned by the key liquidity depth tracker.
 */
export const LiquidityDepthError = {
  /**
   * Price must be strictly positive (> 0).
   */
  1: {message:"InvalidPrice"},
  /**
   * Amount must be strictly positive (> 0).
   */
  2: {message:"InvalidAmount"},
  /**
   * Price level was not found when attempting to cancel depth.
   */
  3: {message:"PriceLevelNotFound"},
  /**
   * Cancellation amount exceeds the available depth volume at the price level.
   */
  4: {message:"InsufficientDepth"},
  /**
   * Arithmetic overflow in volume calculation.
   */
  5: {message:"ArithmeticOverflow"}
}

export interface Client {
  /**
   * Construct and simulate a get_depth transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns top `levels` bid and ask price levels with cumulative volume.
   * 
   * - Bids are sorted in descending order of price (highest bid first).
   * - Asks are sorted in ascending order of price (lowest ask first).
   * - Cumulative volume accumulates from top of book downward.
   */
  get_depth: ({key_id, levels}: {key_id: string, levels: u32}, options?: MethodOptions) => Promise<AssembledTransaction<LiquidityDepth>>

  /**
   * Construct and simulate a get_max_buy transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the largest buy amount whose quoted impact stays within the limit.
   */
  get_max_buy: ({key_id, max_impact_bps}: {key_id: string, max_impact_bps: u32}, options?: MethodOptions) => Promise<AssembledTransaction<LiquidityQuote>>

  /**
   * Construct and simulate a get_max_sell transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the largest sell amount whose quoted impact stays within the limit.
   */
  get_max_sell: ({key_id, min_impact_bps}: {key_id: string, min_impact_bps: u32}, options?: MethodOptions) => Promise<AssembledTransaction<LiquidityQuote>>

  /**
   * Construct and simulate a record_order transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Records an order volume at the specified price level for a creator key.
   * 
   * Called by swap and limit order contracts on order placement.
   * Updates depth atomically and emits a [`DepthUpdated`] event.
   */
  record_order: ({key_id, price, amount, side}: {key_id: string, price: i128, amount: i128, side: OrderSide}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_order_depth transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Removes cancelled or filled order volume from depth at a price level.
   * 
   * Called by swap and limit order contracts on order cancellation or settlement.
   * Updates depth atomically and emits a [`DepthUpdated`] event.
   */
  cancel_order_depth: ({key_id, price, amount, side}: {key_id: string, price: i128, amount: i128, side: OrderSide}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_price_level_volume transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Read-only view of total volume recorded at a specific price level.
   */
  get_price_level_volume: ({key_id, price, side}: {key_id: string, price: i128, side: OrderSide}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

}
export class Client extends ContractClient {
  static async deploy<T = Client>(
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
    return ContractClient.deploy(null, options)
  }
  constructor(public readonly options: ContractClientOptions) {
    super(
      new ContractSpec([ "AAAAAAAAAQdSZXR1cm5zIHRvcCBgbGV2ZWxzYCBiaWQgYW5kIGFzayBwcmljZSBsZXZlbHMgd2l0aCBjdW11bGF0aXZlIHZvbHVtZS4KCi0gQmlkcyBhcmUgc29ydGVkIGluIGRlc2NlbmRpbmcgb3JkZXIgb2YgcHJpY2UgKGhpZ2hlc3QgYmlkIGZpcnN0KS4KLSBBc2tzIGFyZSBzb3J0ZWQgaW4gYXNjZW5kaW5nIG9yZGVyIG9mIHByaWNlIChsb3dlc3QgYXNrIGZpcnN0KS4KLSBDdW11bGF0aXZlIHZvbHVtZSBhY2N1bXVsYXRlcyBmcm9tIHRvcCBvZiBib29rIGRvd253YXJkLgAAAAAJZ2V0X2RlcHRoAAAAAAAAAgAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAZsZXZlbHMAAAAAAAQAAAABAAAH0AAAAA5MaXF1aWRpdHlEZXB0aAAA",
        "AAAAAgAAACJTdG9yYWdlIGtleXMgZm9yIHBlcnNpc3RlbnQgc3RhdGUuAAAAAAAAAAAAB0RhdGFLZXkAAAAAAgAAAAEAAABOQWN0aXZlIHByaWNlIGxldmVscyBsaXN0IGZvciBhIGtleSBhbmQgc2lkZTogKEFkZHJlc3MsIE9yZGVyU2lkZSkgLT4gVmVjPGkxMjg+AAAAAAALUHJpY2VMZXZlbHMAAAAAAgAAABMAAAfQAAAACU9yZGVyU2lkZQAAAAAAAAEAAABEVm9sdW1lIGF0IGEgc3BlY2lmaWMgcHJpY2UgbGV2ZWw6IChBZGRyZXNzLCBPcmRlclNpZGUsIGkxMjgpIC0+IGkxMjgAAAALTGV2ZWxWb2x1bWUAAAAAAwAAABMAAAfQAAAACU9yZGVyU2lkZQAAAAAAAAs=",
        "AAAAAAAAAEpSZXR1cm5zIHRoZSBsYXJnZXN0IGJ1eSBhbW91bnQgd2hvc2UgcXVvdGVkIGltcGFjdCBzdGF5cyB3aXRoaW4gdGhlIGxpbWl0LgAAAAAAC2dldF9tYXhfYnV5AAAAAAIAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAObWF4X2ltcGFjdF9icHMAAAAAAAQAAAABAAAH0AAAAA5MaXF1aWRpdHlRdW90ZQAA",
        "AAAAAwAAACNTaWRlIG9mIGFuIG9yZGVyIGluIHRoZSBvcmRlciBib29rLgAAAAAAAAAACU9yZGVyU2lkZQAAAAAAAAIAAAAqQnV5IG9yZGVyIChCaWQpIC0gYnV5ZXJzIGJpZGRpbmcgZm9yIGtleXMuAAAAAAADQnV5AAAAAAAAAAApU2VsbCBvcmRlciAoQXNrKSAtIHNlbGxlcnMgb2ZmZXJpbmcga2V5cy4AAAAAAAAEU2VsbAAAAAE=",
        "AAAAAAAAAEtSZXR1cm5zIHRoZSBsYXJnZXN0IHNlbGwgYW1vdW50IHdob3NlIHF1b3RlZCBpbXBhY3Qgc3RheXMgd2l0aGluIHRoZSBsaW1pdC4AAAAADGdldF9tYXhfc2VsbAAAAAIAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAObWluX2ltcGFjdF9icHMAAAAAAAQAAAABAAAH0AAAAA5MaXF1aWRpdHlRdW90ZQAA",
        "AAAAAAAAAMJSZWNvcmRzIGFuIG9yZGVyIHZvbHVtZSBhdCB0aGUgc3BlY2lmaWVkIHByaWNlIGxldmVsIGZvciBhIGNyZWF0b3Iga2V5LgoKQ2FsbGVkIGJ5IHN3YXAgYW5kIGxpbWl0IG9yZGVyIGNvbnRyYWN0cyBvbiBvcmRlciBwbGFjZW1lbnQuClVwZGF0ZXMgZGVwdGggYXRvbWljYWxseSBhbmQgZW1pdHMgYSBbYERlcHRoVXBkYXRlZGBdIGV2ZW50LgAAAAAADHJlY29yZF9vcmRlcgAAAAQAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAFcHJpY2UAAAAAAAALAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABHNpZGUAAAfQAAAACU9yZGVyU2lkZQAAAAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAE0xpcXVpZGl0eURlcHRoRXJyb3IA",
        "AAAAAQAAAERBIHNpbmdsZSBwcmljZSBsZXZlbCBpbiB0aGUgb3JkZXIgYm9vayBkZXB0aCB3aXRoIGN1bXVsYXRpdmUgdm9sdW1lLgAAAAAAAAAKRGVwdGhMZXZlbAAAAAAAAwAAADdDdW11bGF0aXZlIHZvbHVtZSB1cCB0byBhbmQgaW5jbHVkaW5nIHRoaXMgcHJpY2UgbGV2ZWwuAAAAABFjdW11bGF0aXZlX3ZvbHVtZQAAAAAAAAsAAAAdUHJpY2UgcGVyIHVuaXQgYXQgdGhpcyBsZXZlbC4AAAAAAAAFcHJpY2UAAAAAAAALAAAALlZvbHVtZSBhdmFpbGFibGUgYXQgdGhpcyBzcGVjaWZpYyBwcmljZSBsZXZlbC4AAAAAAAZ2b2x1bWUAAAAAAAs=",
        "AAAAAQAAAIhFdmVudCBwYXlsb2FkIGVtaXR0ZWQgd2hlbiBvcmRlciBib29rIGRlcHRoIGNoYW5nZXMuCgpFdmVudCBzaGFwZToKLSB0b3BpY3M6IGAoREVQVEhfVVBEQVRFRF9FVkVOVF9OQU1FLCBrZXlfaWQpYAotIGRhdGE6IGBEZXB0aFVwZGF0ZWRgAAAAAAAAAAxEZXB0aFVwZGF0ZWQAAAAFAAAAIE9yZGVyIGFtb3VudCBhZGRlZCBvciBjYW5jZWxsZWQuAAAABmFtb3VudAAAAAAACwAAACZDcmVhdG9yIGtleSB3aG9zZSBvcmRlciBkZXB0aCBjaGFuZ2VkLgAAAAAABmtleV9pZAAAAAAAEwAAABVQcmljZSBsZXZlbCBhZmZlY3RlZC4AAAAAAAAFcHJpY2UAAAAAAAALAAAAGU9yZGVyIHNpZGUgKEJ1eSBvciBTZWxsKS4AAAAAAAAEc2lkZQAAB9AAAAAJT3JkZXJTaWRlAAAAAAAAPFJlbWFpbmluZyB0b3RhbCB2b2x1bWUgYXQgdGhpcyBwcmljZSBsZXZlbCBhZnRlciB0aGUgdXBkYXRlLgAAAAZ2b2x1bWUAAAAAAAs=",
        "AAAAAQAAAD9GdWxsIGxpcXVpZGl0eSBkZXB0aCBzdW1tYXJ5IGNvbnRhaW5pbmcgdG9wIGJpZCBhbmQgYXNrIGxldmVscy4AAAAAAAAAAA5MaXF1aWRpdHlEZXB0aAAAAAAAAgAAADpBc2sgbGV2ZWxzIHNvcnRlZCBpbiBhc2NlbmRpbmcgb3JkZXIgKGxvd2VzdCBwcmljZSBmaXJzdCkuAAAAAAAEYXNrcwAAA+oAAAfQAAAACkRlcHRoTGV2ZWwAAAAAADxCaWQgbGV2ZWxzIHNvcnRlZCBpbiBkZXNjZW5kaW5nIG9yZGVyIChoaWdoZXN0IHByaWNlIGZpcnN0KS4AAAAEYmlkcwAAA+oAAAfQAAAACkRlcHRoTGV2ZWwAAA==",
        "AAAAAQAAAAAAAAAAAAAADkxpcXVpZGl0eVF1b3RlAAAAAAADAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAADmV4cGVjdGVkX3ByaWNlAAAAAAALAAAAAAAAABBwcmljZV9pbXBhY3RfYnBzAAAABA==",
        "AAAAAAAAANFSZW1vdmVzIGNhbmNlbGxlZCBvciBmaWxsZWQgb3JkZXIgdm9sdW1lIGZyb20gZGVwdGggYXQgYSBwcmljZSBsZXZlbC4KCkNhbGxlZCBieSBzd2FwIGFuZCBsaW1pdCBvcmRlciBjb250cmFjdHMgb24gb3JkZXIgY2FuY2VsbGF0aW9uIG9yIHNldHRsZW1lbnQuClVwZGF0ZXMgZGVwdGggYXRvbWljYWxseSBhbmQgZW1pdHMgYSBbYERlcHRoVXBkYXRlZGBdIGV2ZW50LgAAAAAAABJjYW5jZWxfb3JkZXJfZGVwdGgAAAAAAAQAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAFcHJpY2UAAAAAAAALAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAABHNpZGUAAAfQAAAACU9yZGVyU2lkZQAAAAAAAAEAAAPpAAAD7QAAAAAAAAfQAAAAE0xpcXVpZGl0eURlcHRoRXJyb3IA",
        "AAAABAAAADNFcnJvcnMgcmV0dXJuZWQgYnkgdGhlIGtleSBsaXF1aWRpdHkgZGVwdGggdHJhY2tlci4AAAAAAAAAABNMaXF1aWRpdHlEZXB0aEVycm9yAAAAAAUAAAAmUHJpY2UgbXVzdCBiZSBzdHJpY3RseSBwb3NpdGl2ZSAoPiAwKS4AAAAAAAxJbnZhbGlkUHJpY2UAAAABAAAAJ0Ftb3VudCBtdXN0IGJlIHN0cmljdGx5IHBvc2l0aXZlICg+IDApLgAAAAANSW52YWxpZEFtb3VudAAAAAAAAAIAAAA6UHJpY2UgbGV2ZWwgd2FzIG5vdCBmb3VuZCB3aGVuIGF0dGVtcHRpbmcgdG8gY2FuY2VsIGRlcHRoLgAAAAAAElByaWNlTGV2ZWxOb3RGb3VuZAAAAAAAAwAAAEpDYW5jZWxsYXRpb24gYW1vdW50IGV4Y2VlZHMgdGhlIGF2YWlsYWJsZSBkZXB0aCB2b2x1bWUgYXQgdGhlIHByaWNlIGxldmVsLgAAAAAAEUluc3VmZmljaWVudERlcHRoAAAAAAAABAAAACpBcml0aG1ldGljIG92ZXJmbG93IGluIHZvbHVtZSBjYWxjdWxhdGlvbi4AAAAAABJBcml0aG1ldGljT3ZlcmZsb3cAAAAAAAU=",
        "AAAAAAAAAEJSZWFkLW9ubHkgdmlldyBvZiB0b3RhbCB2b2x1bWUgcmVjb3JkZWQgYXQgYSBzcGVjaWZpYyBwcmljZSBsZXZlbC4AAAAAABZnZXRfcHJpY2VfbGV2ZWxfdm9sdW1lAAAAAAADAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAEc2lkZQAAB9AAAAAJT3JkZXJTaWRlAAAAAAAAAQAAAAs=" ]),
      options
    )
  }
  public readonly fromJSON = {
    get_depth: this.txFromJSON<LiquidityDepth>,
        get_max_buy: this.txFromJSON<LiquidityQuote>,
        get_max_sell: this.txFromJSON<LiquidityQuote>,
        record_order: this.txFromJSON<Result<void>>,
        cancel_order_depth: this.txFromJSON<Result<void>>,
        get_price_level_volume: this.txFromJSON<i128>
  }
}