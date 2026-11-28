// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: storage-cleanup
// Interface SHA-256: 09c111ce399ae643e925d8c6ff380b4a021726d2afbeacd5ee92f6984263600e
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




export type RecordKind = {tag: "Balance", values: void} | {tag: "Auction", values: void} | {tag: "Order", values: void} | {tag: "Reservation", values: void} | {tag: "Checkpoint", values: void};

export const CleanupError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"Unauthorized"},
  3: {message:"RecordActive"},
  4: {message:"RecordNotFound"},
  5: {message:"InsufficientFund"},
  6: {message:"ReferencedBySnapshot"},
  7: {message:"ReferencedByClaim"},
  8: {message:"ArithmeticOverflow"}
}


export interface CleanableStatus {
  can_remove: boolean;
  reason: string;
}

export interface Client {
  /**
   * Construct and simulate a cleanable transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cleanable: ({kind, holder, id}: {kind: RecordKind, holder: string, id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<CleanableStatus>>

  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  initialize: ({admin, reward_per_entry, retention_window}: {admin: string, reward_per_entry: i128, retention_window: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a fund_cleanup transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  fund_cleanup: ({funder, amount}: {funder: string, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cleanup_order transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cleanup_order: ({caller, order_id}: {caller: string, order_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cleanup_auction transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cleanup_auction: ({caller, auction_id}: {caller: string, auction_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cleanup_balance transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cleanup_balance: ({caller, holder, key_id}: {caller: string, holder: string, key_id: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cleanup_checkpoint transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cleanup_checkpoint: ({caller, holder, checkpoint_id}: {caller: string, holder: string, checkpoint_id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cleanup_reservation transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cleanup_reservation: ({caller, key_id, buyer}: {caller: string, key_id: string, buyer: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAAJY2xlYW5hYmxlAAAAAAAAAwAAAAAAAAAEa2luZAAAB9AAAAAKUmVjb3JkS2luZAAAAAAAAAAAAAZob2xkZXIAAAAAABMAAAAAAAAAAmlkAAAAAAAGAAAAAQAAB9AAAAAPQ2xlYW5hYmxlU3RhdHVzAA==",
        "AAAAAAAAAAAAAAAKaW5pdGlhbGl6ZQAAAAAAAwAAAAAAAAAFYWRtaW4AAAAAAAATAAAAAAAAABByZXdhcmRfcGVyX2VudHJ5AAAACwAAAAAAAAAQcmV0ZW50aW9uX3dpbmRvdwAAAAYAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAxDbGVhbnVwRXJyb3I=",
        "AAAAAAAAAAAAAAAMZnVuZF9jbGVhbnVwAAAAAgAAAAAAAAAGZnVuZGVyAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAxDbGVhbnVwRXJyb3I=",
        "AAAAAgAAAAAAAAAAAAAAClJlY29yZEtpbmQAAAAAAAUAAAAAAAAAAAAAAAdCYWxhbmNlAAAAAAAAAAAAAAAAB0F1Y3Rpb24AAAAAAAAAAAAAAAAFT3JkZXIAAAAAAAAAAAAAAAAAAAtSZXNlcnZhdGlvbgAAAAAAAAAAAAAAAApDaGVja3BvaW50AAA=",
        "AAAAAAAAAAAAAAANY2xlYW51cF9vcmRlcgAAAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAIb3JkZXJfaWQAAAAGAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAMQ2xlYW51cEVycm9y",
        "AAAABAAAAAAAAAAAAAAADENsZWFudXBFcnJvcgAAAAgAAAAAAAAAEkFscmVhZHlJbml0aWFsaXplZAAAAAAAAQAAAAAAAAAMVW5hdXRob3JpemVkAAAAAgAAAAAAAAAMUmVjb3JkQWN0aXZlAAAAAwAAAAAAAAAOUmVjb3JkTm90Rm91bmQAAAAAAAQAAAAAAAAAEEluc3VmZmljaWVudEZ1bmQAAAAFAAAAAAAAABRSZWZlcmVuY2VkQnlTbmFwc2hvdAAAAAYAAAAAAAAAEVJlZmVyZW5jZWRCeUNsYWltAAAAAAAABwAAAAAAAAASQXJpdGhtZXRpY092ZXJmbG93AAAAAAAI",
        "AAAAAAAAAAAAAAAPY2xlYW51cF9hdWN0aW9uAAAAAAIAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAKYXVjdGlvbl9pZAAAAAAABgAAAAEAAAPpAAAD7QAAAAAAAAfQAAAADENsZWFudXBFcnJvcg==",
        "AAAAAAAAAAAAAAAPY2xlYW51cF9iYWxhbmNlAAAAAAMAAAAAAAAABmNhbGxlcgAAAAAAEwAAAAAAAAAGaG9sZGVyAAAAAAATAAAAAAAAAAZrZXlfaWQAAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAxDbGVhbnVwRXJyb3I=",
        "AAAAAQAAAAAAAAAAAAAAD0NsZWFuYWJsZVN0YXR1cwAAAAACAAAAAAAAAApjYW5fcmVtb3ZlAAAAAAABAAAAAAAAAAZyZWFzb24AAAAAABA=",
        "AAAAAAAAAAAAAAASY2xlYW51cF9jaGVja3BvaW50AAAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABmhvbGRlcgAAAAAAEwAAAAAAAAANY2hlY2twb2ludF9pZAAAAAAAAAYAAAABAAAD6QAAA+0AAAAAAAAH0AAAAAxDbGVhbnVwRXJyb3I=",
        "AAAAAAAAAAAAAAATY2xlYW51cF9yZXNlcnZhdGlvbgAAAAADAAAAAAAAAAZjYWxsZXIAAAAAABMAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAFYnV5ZXIAAAAAAAATAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAAMQ2xlYW51cEVycm9y" ]),
      options
    )
  }
  public readonly fromJSON = {
    cleanable: this.txFromJSON<CleanableStatus>,
        initialize: this.txFromJSON<Result<void>>,
        fund_cleanup: this.txFromJSON<Result<void>>,
        cleanup_order: this.txFromJSON<Result<void>>,
        cleanup_auction: this.txFromJSON<Result<void>>,
        cleanup_balance: this.txFromJSON<Result<void>>,
        cleanup_checkpoint: this.txFromJSON<Result<void>>,
        cleanup_reservation: this.txFromJSON<Result<void>>
  }
}