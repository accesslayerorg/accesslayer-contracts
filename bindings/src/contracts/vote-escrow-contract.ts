// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: vote-escrow-contract
// WASM SHA-256: 2a67dd138662e538fc7419f9ba4f857038797edcec23819f9a9157f4f85739ba
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





export interface Lock {
  amount: i128;
  created_at: u64;
  holder: string;
  id: u64;
  key_id: string;
  unlock_ledger: u64;
}

export const Errors = {
  1: {message:"InvalidAmount"},
  2: {message:"InvalidUnlock"},
  3: {message:"LockNotFound"},
  4: {message:"LockActive"},
  5: {message:"CannotShorten"},
  6: {message:"Overflow"}
}

export interface Client {
  /**
   * Construct and simulate a lock transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  lock: ({holder, key_id, amount, unlock_ledger}: {holder: string, key_id: string, amount: i128, unlock_ledger: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u64>>>

  /**
   * Construct and simulate a get_lock transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_lock: ({id}: {id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Option<Lock>>>

  /**
   * Construct and simulate a withdraw transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  withdraw: ({holder, id}: {holder: string, id: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a extend_lock transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  extend_lock: ({holder, id, new_unlock_ledger}: {holder: string, id: u64, new_unlock_ledger: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a voting_power transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  voting_power: ({id, at_ledger}: {id: u64, at_ledger: u64}, options?: MethodOptions) => Promise<AssembledTransaction<Result<i128>>>

  /**
   * Construct and simulate a increase_lock transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  increase_lock: ({holder, id, amount}: {holder: string, id: u64, amount: i128}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

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
      new ContractSpec([ "AAAAAAAAAAAAAAAEbG9jawAAAAQAAAAAAAAABmhvbGRlcgAAAAAAEwAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAADXVubG9ja19sZWRnZXIAAAAAAAAGAAAAAQAAA+kAAAAGAAAAAw==",
        "AAAAAQAAAAAAAAAAAAAABExvY2sAAAAGAAAAAAAAAAZhbW91bnQAAAAAAAsAAAAAAAAACmNyZWF0ZWRfYXQAAAAAAAYAAAAAAAAABmhvbGRlcgAAAAAAEwAAAAAAAAACaWQAAAAAAAYAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAANdW5sb2NrX2xlZGdlcgAAAAAAAAY=",
        "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABgAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAEAAAAAAAAADUludmFsaWRVbmxvY2sAAAAAAAACAAAAAAAAAAxMb2NrTm90Rm91bmQAAAADAAAAAAAAAApMb2NrQWN0aXZlAAAAAAAEAAAAAAAAAA1DYW5ub3RTaG9ydGVuAAAAAAAABQAAAAAAAAAIT3ZlcmZsb3cAAAAG",
        "AAAAAAAAAAAAAAAIZ2V0X2xvY2sAAAABAAAAAAAAAAJpZAAAAAAABgAAAAEAAAPoAAAH0AAAAARMb2Nr",
        "AAAAAAAAAAAAAAAId2l0aGRyYXcAAAACAAAAAAAAAAZob2xkZXIAAAAAABMAAAAAAAAAAmlkAAAAAAAGAAAAAQAAA+kAAAALAAAAAw==",
        "AAAAAAAAAAAAAAALZXh0ZW5kX2xvY2sAAAAAAwAAAAAAAAAGaG9sZGVyAAAAAAATAAAAAAAAAAJpZAAAAAAABgAAAAAAAAARbmV3X3VubG9ja19sZWRnZXIAAAAAAAAGAAAAAQAAA+kAAAPtAAAAAAAAAAM=",
        "AAAAAAAAAAAAAAAMdm90aW5nX3Bvd2VyAAAAAgAAAAAAAAACaWQAAAAAAAYAAAAAAAAACWF0X2xlZGdlcgAAAAAAAAYAAAABAAAD6QAAAAsAAAAD",
        "AAAAAAAAAAAAAAANaW5jcmVhc2VfbG9jawAAAAAAAAMAAAAAAAAABmhvbGRlcgAAAAAAEwAAAAAAAAACaWQAAAAAAAYAAAAAAAAABmFtb3VudAAAAAAACwAAAAEAAAPpAAAD7QAAAAAAAAAD" ]),
      options
    )
  }
  public readonly fromJSON = {
    lock: this.txFromJSON<Result<u64>>,
        get_lock: this.txFromJSON<Option<Lock>>,
        withdraw: this.txFromJSON<Result<i128>>,
        extend_lock: this.txFromJSON<Result<void>>,
        voting_power: this.txFromJSON<Result<i128>>,
        increase_lock: this.txFromJSON<Result<void>>
  }
}