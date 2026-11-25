// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: sealed-launch-auction
// WASM SHA-256: e96534e373742d2fc8c75f1199c42255f469fb6e479056f43e7ed0a4a32723ad
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





export interface Bid {
  bidder: string;
  commitment: Buffer;
  deposit: i128;
  price: i128;
  quantity: u32;
  revealed: boolean;
}

export const Errors = {
  1: {message:"Unauthorized"},
  2: {message:"NotFound"},
  3: {message:"Phase"},
  4: {message:"InvalidCommitment"},
  5: {message:"AlreadySettled"}
}


export interface Auction {
  bid_end: u32;
  clearing_price: i128;
  creator: string;
  filled: u32;
  key_id: string;
  min_price: i128;
  reveal_end: u32;
  settled: boolean;
  supply: u32;
}

export interface Client {
  /**
   * Construct and simulate a bid transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  bid: ({bidder, auction_id, commitment, deposit}: {bidder: string, auction_id: u32, commitment: Buffer, deposit: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a reveal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  reveal: ({bidder, auction_id, price, quantity, salt}: {bidder: string, auction_id: u32, price: i128, quantity: u32, salt: Buffer}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a settle transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  settle: ({auction_id}: {auction_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a create_sealed_auction transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  create_sealed_auction: ({creator, key_id, supply, min_price, bid_ledgers, reveal_ledgers}: {creator: string, key_id: string, supply: u32, min_price: i128, bid_ledgers: u32, reveal_ledgers: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAADYmlkAAAAAAQAAAAAAAAABmJpZGRlcgAAAAAAEwAAAAAAAAAKYXVjdGlvbl9pZAAAAAAABAAAAAAAAAAKY29tbWl0bWVudAAAAAAADgAAAAAAAAAHZGVwb3NpdAAAAAALAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAQAAAAAAAAAAAAAAA0JpZAAAAAAGAAAAAAAAAAZiaWRkZXIAAAAAABMAAAAAAAAACmNvbW1pdG1lbnQAAAAAAA4AAAAAAAAAB2RlcG9zaXQAAAAACwAAAAAAAAAFcHJpY2UAAAAAAAALAAAAAAAAAAhxdWFudGl0eQAAAAQAAAAAAAAACHJldmVhbGVkAAAAAQ==",
        "AAAAAAAAAAAAAAAGcmV2ZWFsAAAAAAAFAAAAAAAAAAZiaWRkZXIAAAAAABMAAAAAAAAACmF1Y3Rpb25faWQAAAAAAAQAAAAAAAAABXByaWNlAAAAAAAACwAAAAAAAAAIcXVhbnRpdHkAAAAEAAAAAAAAAARzYWx0AAAADgAAAAEAAAPpAAAD7QAAAAAAAAAD",
        "AAAAAAAAAAAAAAAGc2V0dGxlAAAAAAABAAAAAAAAAAphdWN0aW9uX2lkAAAAAAAEAAAAAQAAA+kAAAALAAAAAw==",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABQAAAAAAAAAMVW5hdXRob3JpemVkAAAAAQAAAAAAAAAITm90Rm91bmQAAAACAAAAAAAAAAVQaGFzZQAAAAAAAAMAAAAAAAAAEUludmFsaWRDb21taXRtZW50AAAAAAAABAAAAAAAAAAOQWxyZWFkeVNldHRsZWQAAAAAAAU=",
        "AAAAAQAAAAAAAAAAAAAAB0F1Y3Rpb24AAAAACQAAAAAAAAAHYmlkX2VuZAAAAAAEAAAAAAAAAA5jbGVhcmluZ19wcmljZQAAAAAACwAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAZmaWxsZWQAAAAAAAQAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAJbWluX3ByaWNlAAAAAAAACwAAAAAAAAAKcmV2ZWFsX2VuZAAAAAAABAAAAAAAAAAHc2V0dGxlZAAAAAABAAAAAAAAAAZzdXBwbHkAAAAAAAQ=",
        "AAAAAAAAAAAAAAAVY3JlYXRlX3NlYWxlZF9hdWN0aW9uAAAAAAAABgAAAAAAAAAHY3JlYXRvcgAAAAATAAAAAAAAAAZrZXlfaWQAAAAAABMAAAAAAAAABnN1cHBseQAAAAAABAAAAAAAAAAJbWluX3ByaWNlAAAAAAAACwAAAAAAAAALYmlkX2xlZGdlcnMAAAAABAAAAAAAAAAOcmV2ZWFsX2xlZGdlcnMAAAAAAAQAAAABAAAD6QAAAAQAAAAD" ]),
      options
    )
  }
  public readonly fromJSON = {
    bid: this.txFromJSON<Result<void>>,
        reveal: this.txFromJSON<Result<void>>,
        settle: this.txFromJSON<Result<i128>>,
        create_sealed_auction: this.txFromJSON<Result<u32>>
  }
}