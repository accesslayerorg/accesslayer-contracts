// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: launch-auction
// Interface SHA-256: 4fe2a3798b2db0261f3ecea46bde7a75ade3db43a0f20ad1989111012699e547
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
 * Individual sealed bid placed by a participant.
 */
export interface Bid {
  amount: i128;
  auction_id: u32;
  bidder: string;
  id: u32;
  timestamp: u64;
}


/**
 * Metadata and state for a key launch auction.
 */
export interface Auction {
  clearing_price: i128;
  creator: string;
  end_time: u64;
  id: u32;
  key_id: string;
  keys_allocated: u32;
  min_bid: i128;
  settled: boolean;
  start_time: u64;
  supply: u32;
  total_raised: i128;
}

export type DataKey = {tag: "AuctionCount", values: void} | {tag: "Auction", values: readonly [u32]} | {tag: "AuctionBids", values: readonly [u32]} | {tag: "Escrow", values: readonly [u32, string]} | {tag: "Allocation", values: readonly [u32, string]} | {tag: "ClearingPrice", values: readonly [u32]} | {tag: "BondingCurveOpen", values: readonly [u32]};

export const AuctionError = {
  /**
   * Bid amount is below the auction's minimum required bid.
   */
  1: {message:"BelowMinimum"},
  /**
   * Specified auction was not found.
   */
  2: {message:"AuctionNotFound"},
  /**
   * Auction has closed and no longer accepts bids.
   */
  3: {message:"AuctionExpired"},
  /**
   * Auction duration has not ended; cannot settle before close.
   */
  4: {message:"AuctionNotClosed"},
  /**
   * Auction has already been settled.
   */
  5: {message:"AlreadySettled"},
  /**
   * Caller is not authorized to perform this operation.
   */
  6: {message:"Unauthorized"},
  /**
   * Supply must be strictly greater than zero.
   */
  7: {message:"InvalidSupply"},
  /**
   * Auction duration must be strictly greater than zero.
   */
  8: {message:"InvalidDuration"},
  /**
   * Minimum bid must be strictly greater than zero.
   */
  9: {message:"InvalidMinBid"}
}


/**
 * Event payload for `BidPlaced`.
 */
export interface BidPlacedEvent {
  amount: i128;
  auction_id: u32;
  bid_id: u32;
  bidder: string;
  timestamp: u64;
}


/**
 * Event payload for `AuctionCreated`.
 */
export interface AuctionCreatedEvent {
  auction_id: u32;
  creator: string;
  duration_seconds: u64;
  end_time: u64;
  key_id: string;
  min_bid: i128;
  start_time: u64;
  supply: u32;
}


/**
 * Event payload for `AuctionSettled`.
 */
export interface AuctionSettledEvent {
  auction_id: u32;
  clearing_price: i128;
  key_id: string;
  keys_allocated: u32;
  refunded_bids_count: u32;
  settled_at: u64;
  supply: u32;
  total_raised: i128;
  winning_bids_count: u32;
}

export interface Client {
  /**
   * Construct and simulate a get_bids transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns all bids submitted for an auction.
   */
  get_bids: ({auction_id}: {auction_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Array<Bid>>>

  /**
   * Construct and simulate a place_bid transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Places a sealed bid in an open launch auction, locking funds in contract escrow.
   * 
   * Rejects bids below `min_bid` with [`AuctionError::BelowMinimum`].
   */
  place_bid: ({bidder, auction_id, amount}: {bidder: string, auction_id: u32, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a get_auction transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns auction details by ID.
   */
  get_auction: ({auction_id}: {auction_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Auction>>>

  /**
   * Construct and simulate a create_auction transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Creates a sealed-bid launch auction for a creator key's initial allocation.
   * 
   * Creator-only at launch: requires authentication from `key_id`.
   */
  create_auction: ({key_id, supply, duration_seconds, min_bid}: {key_id: string, supply: u32, duration_seconds: u64, min_bid: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a get_allocation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the number of keys allocated to a bidder for a settled auction.
   */
  get_allocation: ({auction_id, bidder}: {auction_id: u32, bidder: string}, options?: MethodOptions) => Promise<AssembledTransaction<u32>>

  /**
   * Construct and simulate a settle_auction transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Settles the auction after duration ends:
   * 1. Clears bids from highest to lowest downward until supply is exhausted.
   * 2. Automatically refunds losing bids (and excess over clearing price).
   * 3. Establishes the initial clearing price for price discovery.
   * 4. Opens the bonding curve at the discovered clearing price.
   * 5. Emits `AuctionSettled` with complete auction outcome data.
   */
  settle_auction: ({auction_id}: {auction_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a get_clearing_price transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the initial clearing price discovered for an auction.
   */
  get_clearing_price: ({auction_id}: {auction_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Option<i128>>>

  /**
   * Construct and simulate a get_escrow_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns the remaining locked escrow balance for a bidder in an auction.
   */
  get_escrow_balance: ({auction_id, bidder}: {auction_id: u32, bidder: string}, options?: MethodOptions) => Promise<AssembledTransaction<i128>>

  /**
   * Construct and simulate a is_bonding_curve_open transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Returns whether the bonding curve has opened for an auction's key.
   */
  is_bonding_curve_open: ({auction_id}: {auction_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<boolean>>

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
      new ContractSpec([ "AAAAAQAAAC5JbmRpdmlkdWFsIHNlYWxlZCBiaWQgcGxhY2VkIGJ5IGEgcGFydGljaXBhbnQuAAAAAAAAAAAAA0JpZAAAAAAFAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAACmF1Y3Rpb25faWQAAAAAAAQAAAAAAAAABmJpZGRlcgAAAAAAEwAAAAAAAAACaWQAAAAAAAQAAAAAAAAACXRpbWVzdGFtcAAAAAAAAAY=",
        "AAAAAAAAACpSZXR1cm5zIGFsbCBiaWRzIHN1Ym1pdHRlZCBmb3IgYW4gYXVjdGlvbi4AAAAAAAhnZXRfYmlkcwAAAAEAAAAAAAAACmF1Y3Rpb25faWQAAAAAAAQAAAABAAAD6gAAB9AAAAADQmlkAA==",
        "AAAAAAAAAJNQbGFjZXMgYSBzZWFsZWQgYmlkIGluIGFuIG9wZW4gbGF1bmNoIGF1Y3Rpb24sIGxvY2tpbmcgZnVuZHMgaW4gY29udHJhY3QgZXNjcm93LgoKUmVqZWN0cyBiaWRzIGJlbG93IGBtaW5fYmlkYCB3aXRoIFtgQXVjdGlvbkVycm9yOjpCZWxvd01pbmltdW1gXS4AAAAACXBsYWNlX2JpZAAAAAAAAAMAAAAAAAAABmJpZGRlcgAAAAAAEwAAAAAAAAAKYXVjdGlvbl9pZAAAAAAABAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAQAAA+kAAAAEAAAH0AAAAAxBdWN0aW9uRXJyb3I=",
        "AAAAAQAAACxNZXRhZGF0YSBhbmQgc3RhdGUgZm9yIGEga2V5IGxhdW5jaCBhdWN0aW9uLgAAAAAAAAAHQXVjdGlvbgAAAAALAAAAAAAAAA5jbGVhcmluZ19wcmljZQAAAAAACwAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAhlbmRfdGltZQAAAAYAAAAAAAAAAmlkAAAAAAAEAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAADmtleXNfYWxsb2NhdGVkAAAAAAAEAAAAAAAAAAdtaW5fYmlkAAAAAAsAAAAAAAAAB3NldHRsZWQAAAAAAQAAAAAAAAAKc3RhcnRfdGltZQAAAAAABgAAAAAAAAAGc3VwcGx5AAAAAAAEAAAAAAAAAAx0b3RhbF9yYWlzZWQAAAAL",
        "AAAAAgAAAAAAAAAAAAAAB0RhdGFLZXkAAAAABwAAAAAAAAAgVG90YWwgY291bnQgb2YgYXVjdGlvbnMgY3JlYXRlZC4AAAAMQXVjdGlvbkNvdW50AAAAAQAAACZBdWN0aW9uIG1ldGFkYXRhIHJlY29yZCBieSBhdWN0aW9uIElELgAAAAAAB0F1Y3Rpb24AAAAAAQAAAAQAAAABAAAAKUxpc3Qgb2YgYmlkcyBzdWJtaXR0ZWQgZm9yIGFuIGF1Y3Rpb24gSUQuAAAAAAAAC0F1Y3Rpb25CaWRzAAAAAAEAAAAEAAAAAQAAADNFc2Nyb3cgYmFsYW5jZSBsb2NrZWQgcGVyIGJpZGRlciBmb3IgYW4gYXVjdGlvbiBJRC4AAAAABkVzY3JvdwAAAAAAAgAAAAQAAAATAAAAAQAAACRBbGxvY2F0ZWQga2V5IGNvdW50IHdvbiBieSBhIGJpZGRlci4AAAAKQWxsb2NhdGlvbgAAAAAAAgAAAAQAAAATAAAAAQAAADJNYXJrZXQgY2xlYXJpbmcgcHJpY2UgZXN0YWJsaXNoZWQgdXBvbiBzZXR0bGVtZW50LgAAAAAADUNsZWFyaW5nUHJpY2UAAAAAAAABAAAABAAAAAEAAAA6RmxhZyBpbmRpY2F0aW5nIHdoZXRoZXIgdGhlIGJvbmRpbmcgY3VydmUgaGFzIGJlZW4gb3BlbmVkLgAAAAAAEEJvbmRpbmdDdXJ2ZU9wZW4AAAABAAAABA==",
        "AAAAAAAAAB5SZXR1cm5zIGF1Y3Rpb24gZGV0YWlscyBieSBJRC4AAAAAAAtnZXRfYXVjdGlvbgAAAAABAAAAAAAAAAphdWN0aW9uX2lkAAAAAAAEAAAAAQAAA+gAAAfQAAAAB0F1Y3Rpb24A",
        "AAAAAAAAAItDcmVhdGVzIGEgc2VhbGVkLWJpZCBsYXVuY2ggYXVjdGlvbiBmb3IgYSBjcmVhdG9yIGtleSdzIGluaXRpYWwgYWxsb2NhdGlvbi4KCkNyZWF0b3Itb25seSBhdCBsYXVuY2g6IHJlcXVpcmVzIGF1dGhlbnRpY2F0aW9uIGZyb20gYGtleV9pZGAuAAAAAA5jcmVhdGVfYXVjdGlvbgAAAAAABAAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAZzdXBwbHkAAAAAAAQAAAAAAAAAEGR1cmF0aW9uX3NlY29uZHMAAAAGAAAAAAAAAAdtaW5fYmlkAAAAAAsAAAABAAAD6QAAAAQAAAfQAAAADEF1Y3Rpb25FcnJvcg==",
        "AAAAAAAAAEdSZXR1cm5zIHRoZSBudW1iZXIgb2Yga2V5cyBhbGxvY2F0ZWQgdG8gYSBiaWRkZXIgZm9yIGEgc2V0dGxlZCBhdWN0aW9uLgAAAAAOZ2V0X2FsbG9jYXRpb24AAAAAAAIAAAAAAAAACmF1Y3Rpb25faWQAAAAAAAQAAAAAAAAABmJpZGRlcgAAAAAAEwAAAAEAAAAE",
        "AAAAAAAAAXNTZXR0bGVzIHRoZSBhdWN0aW9uIGFmdGVyIGR1cmF0aW9uIGVuZHM6CjEuIENsZWFycyBiaWRzIGZyb20gaGlnaGVzdCB0byBsb3dlc3QgZG93bndhcmQgdW50aWwgc3VwcGx5IGlzIGV4aGF1c3RlZC4KMi4gQXV0b21hdGljYWxseSByZWZ1bmRzIGxvc2luZyBiaWRzIChhbmQgZXhjZXNzIG92ZXIgY2xlYXJpbmcgcHJpY2UpLgozLiBFc3RhYmxpc2hlcyB0aGUgaW5pdGlhbCBjbGVhcmluZyBwcmljZSBmb3IgcHJpY2UgZGlzY292ZXJ5Lgo0LiBPcGVucyB0aGUgYm9uZGluZyBjdXJ2ZSBhdCB0aGUgZGlzY292ZXJlZCBjbGVhcmluZyBwcmljZS4KNS4gRW1pdHMgYEF1Y3Rpb25TZXR0bGVkYCB3aXRoIGNvbXBsZXRlIGF1Y3Rpb24gb3V0Y29tZSBkYXRhLgAAAAAOc2V0dGxlX2F1Y3Rpb24AAAAAAAEAAAAAAAAACmF1Y3Rpb25faWQAAAAAAAQAAAABAAAD6QAAAAsAAAfQAAAADEF1Y3Rpb25FcnJvcg==",
        "AAAABAAAAAAAAAAAAAAADEF1Y3Rpb25FcnJvcgAAAAkAAAA3QmlkIGFtb3VudCBpcyBiZWxvdyB0aGUgYXVjdGlvbidzIG1pbmltdW0gcmVxdWlyZWQgYmlkLgAAAAAMQmVsb3dNaW5pbXVtAAAAAQAAACBTcGVjaWZpZWQgYXVjdGlvbiB3YXMgbm90IGZvdW5kLgAAAA9BdWN0aW9uTm90Rm91bmQAAAAAAgAAAC5BdWN0aW9uIGhhcyBjbG9zZWQgYW5kIG5vIGxvbmdlciBhY2NlcHRzIGJpZHMuAAAAAAAOQXVjdGlvbkV4cGlyZWQAAAAAAAMAAAA7QXVjdGlvbiBkdXJhdGlvbiBoYXMgbm90IGVuZGVkOyBjYW5ub3Qgc2V0dGxlIGJlZm9yZSBjbG9zZS4AAAAAEEF1Y3Rpb25Ob3RDbG9zZWQAAAAEAAAAIUF1Y3Rpb24gaGFzIGFscmVhZHkgYmVlbiBzZXR0bGVkLgAAAAAAAA5BbHJlYWR5U2V0dGxlZAAAAAAABQAAADNDYWxsZXIgaXMgbm90IGF1dGhvcml6ZWQgdG8gcGVyZm9ybSB0aGlzIG9wZXJhdGlvbi4AAAAADFVuYXV0aG9yaXplZAAAAAYAAAAqU3VwcGx5IG11c3QgYmUgc3RyaWN0bHkgZ3JlYXRlciB0aGFuIHplcm8uAAAAAAANSW52YWxpZFN1cHBseQAAAAAAAAcAAAA0QXVjdGlvbiBkdXJhdGlvbiBtdXN0IGJlIHN0cmljdGx5IGdyZWF0ZXIgdGhhbiB6ZXJvLgAAAA9JbnZhbGlkRHVyYXRpb24AAAAACAAAAC9NaW5pbXVtIGJpZCBtdXN0IGJlIHN0cmljdGx5IGdyZWF0ZXIgdGhhbiB6ZXJvLgAAAAANSW52YWxpZE1pbkJpZAAAAAAAAAk=",
        "AAAAAQAAAB5FdmVudCBwYXlsb2FkIGZvciBgQmlkUGxhY2VkYC4AAAAAAAAAAAAOQmlkUGxhY2VkRXZlbnQAAAAAAAUAAAAAAAAABmFtb3VudAAAAAAACwAAAAAAAAAKYXVjdGlvbl9pZAAAAAAABAAAAAAAAAAGYmlkX2lkAAAAAAAEAAAAAAAAAAZiaWRkZXIAAAAAABMAAAAAAAAACXRpbWVzdGFtcAAAAAAAAAY=",
        "AAAAAAAAAD1SZXR1cm5zIHRoZSBpbml0aWFsIGNsZWFyaW5nIHByaWNlIGRpc2NvdmVyZWQgZm9yIGFuIGF1Y3Rpb24uAAAAAAAAEmdldF9jbGVhcmluZ19wcmljZQAAAAAAAQAAAAAAAAAKYXVjdGlvbl9pZAAAAAAABAAAAAEAAAPoAAAACw==",
        "AAAAAAAAAEdSZXR1cm5zIHRoZSByZW1haW5pbmcgbG9ja2VkIGVzY3JvdyBiYWxhbmNlIGZvciBhIGJpZGRlciBpbiBhbiBhdWN0aW9uLgAAAAASZ2V0X2VzY3Jvd19iYWxhbmNlAAAAAAACAAAAAAAAAAphdWN0aW9uX2lkAAAAAAAEAAAAAAAAAAZiaWRkZXIAAAAAABMAAAABAAAACw==",
        "AAAAAAAAAEJSZXR1cm5zIHdoZXRoZXIgdGhlIGJvbmRpbmcgY3VydmUgaGFzIG9wZW5lZCBmb3IgYW4gYXVjdGlvbidzIGtleS4AAAAAABVpc19ib25kaW5nX2N1cnZlX29wZW4AAAAAAAABAAAAAAAAAAphdWN0aW9uX2lkAAAAAAAEAAAAAQAAAAE=",
        "AAAAAQAAACNFdmVudCBwYXlsb2FkIGZvciBgQXVjdGlvbkNyZWF0ZWRgLgAAAAAAAAAAE0F1Y3Rpb25DcmVhdGVkRXZlbnQAAAAACAAAAAAAAAAKYXVjdGlvbl9pZAAAAAAABAAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAABBkdXJhdGlvbl9zZWNvbmRzAAAABgAAAAAAAAAIZW5kX3RpbWUAAAAGAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAAB21pbl9iaWQAAAAACwAAAAAAAAAKc3RhcnRfdGltZQAAAAAABgAAAAAAAAAGc3VwcGx5AAAAAAAE",
        "AAAAAQAAACNFdmVudCBwYXlsb2FkIGZvciBgQXVjdGlvblNldHRsZWRgLgAAAAAAAAAAE0F1Y3Rpb25TZXR0bGVkRXZlbnQAAAAACQAAAAAAAAAKYXVjdGlvbl9pZAAAAAAABAAAAAAAAAAOY2xlYXJpbmdfcHJpY2UAAAAAAAsAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAOa2V5c19hbGxvY2F0ZWQAAAAAAAQAAAAAAAAAE3JlZnVuZGVkX2JpZHNfY291bnQAAAAABAAAAAAAAAAKc2V0dGxlZF9hdAAAAAAABgAAAAAAAAAGc3VwcGx5AAAAAAAEAAAAAAAAAAx0b3RhbF9yYWlzZWQAAAALAAAAAAAAABJ3aW5uaW5nX2JpZHNfY291bnQAAAAAAAQ=" ]),
      options
    )
  }
  public readonly fromJSON = {
    get_bids: this.txFromJSON<Array<Bid>>,
        place_bid: this.txFromJSON<Result<u32>>,
        get_auction: this.txFromJSON<Option<Auction>>,
        create_auction: this.txFromJSON<Result<u32>>,
        get_allocation: this.txFromJSON<u32>,
        settle_auction: this.txFromJSON<Result<i128>>,
        get_clearing_price: this.txFromJSON<Option<i128>>,
        get_escrow_balance: this.txFromJSON<i128>,
        is_bonding_curve_open: this.txFromJSON<boolean>
  }
}