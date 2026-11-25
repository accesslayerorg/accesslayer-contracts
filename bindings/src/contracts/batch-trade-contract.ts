// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: batch-trade-contract
// WASM SHA-256: 3c13c0d284e5a3824eaa1d1256859d7b878a72316dbccd7512f93426ae41adfc
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




export const Errors = {
  1: {message:"TooManyItems"},
  2: {message:"InvalidAmount"},
  3: {message:"SlippageExceeded"},
  4: {message:"Overflow"}
}


export interface SwapItem {
  amount: i128;
  direction: Direction;
  key_id: string;
  max_slippage_bps: u32;
}

export enum Direction {
  Buy = 0,
  Sell = 1,
}


export interface KeyMarket {
  buy_price: i128;
  fee_bps: u32;
  liquidity: i128;
  sell_price: i128;
}


export interface SwapResult {
  fee: i128;
  input: i128;
  key_id: string;
  output: i128;
  price_impact_bps: u32;
}

export interface Client {
  /**
   * Construct and simulate a batch_swap transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  batch_swap: ({caller, items}: {caller: string, items: Array<SwapItem>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<SwapResult>>>>

  /**
   * Construct and simulate a set_market transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  set_market: ({key, market}: {key: string, market: KeyMarket}, options?: MethodOptions) => Promise<AssembledTransaction<null>>

  /**
   * Construct and simulate a get_batch_quote transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_batch_quote: ({items}: {items: Array<SwapItem>}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<SwapResult>>>>

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
      new ContractSpec([ "AAAABAAAAAAAAAAAAAAABUVycm9yAAAAAAAABAAAAAAAAAAMVG9vTWFueUl0ZW1zAAAAAQAAAAAAAAANSW52YWxpZEFtb3VudAAAAAAAAAIAAAAAAAAAEFNsaXBwYWdlRXhjZWVkZWQAAAADAAAAAAAAAAhPdmVyZmxvdwAAAAQ=",
        "AAAAAAAAAAAAAAAKYmF0Y2hfc3dhcAAAAAAAAgAAAAAAAAAGY2FsbGVyAAAAAAATAAAAAAAAAAVpdGVtcwAAAAAAA+oAAAfQAAAACFN3YXBJdGVtAAAAAQAAA+kAAAPqAAAH0AAAAApTd2FwUmVzdWx0AAAAAAAD",
        "AAAAAAAAAAAAAAAKc2V0X21hcmtldAAAAAAAAgAAAAAAAAADa2V5AAAAABMAAAAAAAAABm1hcmtldAAAAAAH0AAAAAlLZXlNYXJrZXQAAAAAAAAA",
        "AAAAAQAAAAAAAAAAAAAACFN3YXBJdGVtAAAABAAAAAAAAAAGYW1vdW50AAAAAAALAAAAAAAAAAlkaXJlY3Rpb24AAAAAAAfQAAAACURpcmVjdGlvbgAAAAAAAAAAAAAGa2V5X2lkAAAAAAATAAAAAAAAABBtYXhfc2xpcHBhZ2VfYnBzAAAABA==",
        "AAAAAwAAAAAAAAAAAAAACURpcmVjdGlvbgAAAAAAAAIAAAAAAAAAA0J1eQAAAAAAAAAAAAAAAARTZWxsAAAAAQ==",
        "AAAAAQAAAAAAAAAAAAAACUtleU1hcmtldAAAAAAAAAQAAAAAAAAACWJ1eV9wcmljZQAAAAAAAAsAAAAAAAAAB2ZlZV9icHMAAAAABAAAAAAAAAAJbGlxdWlkaXR5AAAAAAAACwAAAAAAAAAKc2VsbF9wcmljZQAAAAAACw==",
        "AAAAAQAAAAAAAAAAAAAAClN3YXBSZXN1bHQAAAAAAAUAAAAAAAAAA2ZlZQAAAAALAAAAAAAAAAVpbnB1dAAAAAAAAAsAAAAAAAAABmtleV9pZAAAAAAAEwAAAAAAAAAGb3V0cHV0AAAAAAALAAAAAAAAABBwcmljZV9pbXBhY3RfYnBzAAAABA==",
        "AAAAAAAAAAAAAAAPZ2V0X2JhdGNoX3F1b3RlAAAAAAEAAAAAAAAABWl0ZW1zAAAAAAAD6gAAB9AAAAAIU3dhcEl0ZW0AAAABAAAD6QAAA+oAAAfQAAAAClN3YXBSZXN1bHQAAAAAAAM=" ]),
      options
    )
  }
  public readonly fromJSON = {
    batch_swap: this.txFromJSON<Result<Array<SwapResult>>>,
        set_market: this.txFromJSON<null>,
        get_batch_quote: this.txFromJSON<Result<Array<SwapResult>>>
  }
}