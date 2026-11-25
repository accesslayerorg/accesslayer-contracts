// AUTO-GENERATED FILE - DO NOT EDIT.
// Source contract: multisig-admin
// WASM SHA-256: 9f4870f6ced321de4481e5df83bcbb56105144aa7bb0b4d932df096c3e00dd46
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





export interface Proposal {
  action: ProposalAction;
  approvals: u32;
  executed: boolean;
  id: u32;
  proposer: string;
  rejected: boolean;
  /**
 * Approval membership and threshold are snapshotted at proposal time.
 */
signers: Array<string>;
  threshold: u32;
}

export type ActionKind = {tag: "Freeze", values: void} | {tag: "Unfreeze", values: void} | {tag: "AclChange", values: void} | {tag: "CurveMigration", values: void};

export const MultisigError = {
  1: {message:"AlreadyInitialized"},
  2: {message:"NotInitialized"},
  3: {message:"Unauthorized"},
  4: {message:"InvalidSignerSet"},
  5: {message:"ProposalNotFound"},
  6: {message:"AlreadyVoted"},
  7: {message:"ProposalRejected"},
  8: {message:"ThresholdNotReached"},
  9: {message:"AlreadyExecuted"},
  10: {message:"InvalidAction"},
  11: {message:"TimelockActive"},
  12: {message:"NoPendingUpdate"}
}

export type ProposalAction = {tag: "ContractCall", values: readonly [ContractCallAction]} | {tag: "UpdateSigners", values: readonly [UpdateSignersAction]};


/**
 * A call proposal targets a contract method which must authorize this multisig
 * contract as its privileged actor. Signer updates use the same approval flow.
 */
export interface ContractCallAction {
  args: Array<any>;
  function: string;
  kind: ActionKind;
  target: string;
}


export interface UpdateSignersAction {
  signers: Array<string>;
  threshold: u32;
}

export interface Client {
  /**
   * Construct and simulate a initialize transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * One-time initialization of the signer set and approval threshold.
   */
  initialize: ({initializer, signers, threshold}: {initializer: string, signers: Array<string>, threshold: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_signers transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_signers: (options?: MethodOptions) => Promise<AssembledTransaction<Result<Array<string>>>>

  /**
   * Construct and simulate a sign_action transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Records one approval per authorized signer.
   */
  sign_action: ({signer, proposal_id}: {signer: string, proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_proposal: ({proposal_id}: {proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<Proposal>>>

  /**
   * Construct and simulate a get_threshold transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_threshold: (options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a execute_action transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Executes an approved proposal. Call proposals invoke the target as this
   * contract, allowing targets to require authorization from the multisig.
   * Execution is permissionless; `executor` is authenticated for auditability.
   */
  execute_action: ({executor, proposal_id}: {executor: string, proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a propose_action transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * Any configured signer may propose a privileged call or a signer-set update.
   */
  propose_action: ({proposer, action}: {proposer: string, action: ProposalAction}, options?: MethodOptions) => Promise<AssembledTransaction<Result<u32>>>

  /**
   * Construct and simulate a reject_proposal transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   * A single authorized rejection is a permanent veto for the proposal.
   */
  reject_proposal: ({signer, proposal_id}: {signer: string, proposal_id: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a cancel_threshold_update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  cancel_threshold_update: ({who}: {who: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a execute_threshold_update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  execute_threshold_update: ({who}: {who: string}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a propose_threshold_update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  propose_threshold_update: (args: {who: string, new: u32}, options?: MethodOptions) => Promise<AssembledTransaction<Result<void>>>

  /**
   * Construct and simulate a get_pending_threshold_update transaction. Returns an `AssembledTransaction` object which will have a `result` field containing the result of the simulation. If this transaction changes contract state, you will need to call `signAndSend()` on the returned object.
   */
  get_pending_threshold_update: (options?: MethodOptions) => Promise<AssembledTransaction<Result<readonly [u32, u64]>>>

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
      new ContractSpec([ "AAAAAAAAAEFPbmUtdGltZSBpbml0aWFsaXphdGlvbiBvZiB0aGUgc2lnbmVyIHNldCBhbmQgYXBwcm92YWwgdGhyZXNob2xkLgAAAAAAAAppbml0aWFsaXplAAAAAAADAAAAAAAAAAtpbml0aWFsaXplcgAAAAATAAAAAAAAAAdzaWduZXJzAAAAA+oAAAATAAAAAAAAAAl0aHJlc2hvbGQAAAAAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANTXVsdGlzaWdFcnJvcgAAAA==",
        "AAAAAQAAAAAAAAAAAAAACFByb3Bvc2FsAAAACAAAAAAAAAAGYWN0aW9uAAAAAAfQAAAADlByb3Bvc2FsQWN0aW9uAAAAAAAAAAAACWFwcHJvdmFscwAAAAAAAAQAAAAAAAAACGV4ZWN1dGVkAAAAAQAAAAAAAAACaWQAAAAAAAQAAAAAAAAACHByb3Bvc2VyAAAAEwAAAAAAAAAIcmVqZWN0ZWQAAAABAAAAQ0FwcHJvdmFsIG1lbWJlcnNoaXAgYW5kIHRocmVzaG9sZCBhcmUgc25hcHNob3R0ZWQgYXQgcHJvcG9zYWwgdGltZS4AAAAAB3NpZ25lcnMAAAAD6gAAABMAAAAAAAAACXRocmVzaG9sZAAAAAAAAAQ=",
        "AAAAAAAAAAAAAAALZ2V0X3NpZ25lcnMAAAAAAAAAAAEAAAPpAAAD6gAAABMAAAfQAAAADU11bHRpc2lnRXJyb3IAAAA=",
        "AAAAAAAAACtSZWNvcmRzIG9uZSBhcHByb3ZhbCBwZXIgYXV0aG9yaXplZCBzaWduZXIuAAAAAAtzaWduX2FjdGlvbgAAAAACAAAAAAAAAAZzaWduZXIAAAAAABMAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1NdWx0aXNpZ0Vycm9yAAAA",
        "AAAAAAAAAAAAAAAMZ2V0X3Byb3Bvc2FsAAAAAQAAAAAAAAALcHJvcG9zYWxfaWQAAAAABAAAAAEAAAPpAAAH0AAAAAhQcm9wb3NhbAAAB9AAAAANTXVsdGlzaWdFcnJvcgAAAA==",
        "AAAAAgAAAAAAAAAAAAAACkFjdGlvbktpbmQAAAAAAAQAAAAAAAAAAAAAAAZGcmVlemUAAAAAAAAAAAAAAAAACFVuZnJlZXplAAAAAAAAAAAAAAAJQWNsQ2hhbmdlAAAAAAAAAAAAAAAAAAAOQ3VydmVNaWdyYXRpb24AAA==",
        "AAAAAAAAAAAAAAANZ2V0X3RocmVzaG9sZAAAAAAAAAAAAAABAAAD6QAAAAQAAAfQAAAADU11bHRpc2lnRXJyb3IAAAA=",
        "AAAAAAAAANlFeGVjdXRlcyBhbiBhcHByb3ZlZCBwcm9wb3NhbC4gQ2FsbCBwcm9wb3NhbHMgaW52b2tlIHRoZSB0YXJnZXQgYXMgdGhpcwpjb250cmFjdCwgYWxsb3dpbmcgdGFyZ2V0cyB0byByZXF1aXJlIGF1dGhvcml6YXRpb24gZnJvbSB0aGUgbXVsdGlzaWcuCkV4ZWN1dGlvbiBpcyBwZXJtaXNzaW9ubGVzczsgYGV4ZWN1dG9yYCBpcyBhdXRoZW50aWNhdGVkIGZvciBhdWRpdGFiaWxpdHkuAAAAAAAADmV4ZWN1dGVfYWN0aW9uAAAAAAACAAAAAAAAAAhleGVjdXRvcgAAABMAAAAAAAAAC3Byb3Bvc2FsX2lkAAAAAAQAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1NdWx0aXNpZ0Vycm9yAAAA",
        "AAAAAAAAAEtBbnkgY29uZmlndXJlZCBzaWduZXIgbWF5IHByb3Bvc2UgYSBwcml2aWxlZ2VkIGNhbGwgb3IgYSBzaWduZXItc2V0IHVwZGF0ZS4AAAAADnByb3Bvc2VfYWN0aW9uAAAAAAACAAAAAAAAAAhwcm9wb3NlcgAAABMAAAAAAAAABmFjdGlvbgAAAAAH0AAAAA5Qcm9wb3NhbEFjdGlvbgAAAAAAAQAAA+kAAAAEAAAH0AAAAA1NdWx0aXNpZ0Vycm9yAAAA",
        "AAAAAAAAAENBIHNpbmdsZSBhdXRob3JpemVkIHJlamVjdGlvbiBpcyBhIHBlcm1hbmVudCB2ZXRvIGZvciB0aGUgcHJvcG9zYWwuAAAAAA9yZWplY3RfcHJvcG9zYWwAAAAAAgAAAAAAAAAGc2lnbmVyAAAAAAATAAAAAAAAAAtwcm9wb3NhbF9pZAAAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANTXVsdGlzaWdFcnJvcgAAAA==",
        "AAAABAAAAAAAAAAAAAAADU11bHRpc2lnRXJyb3IAAAAAAAAMAAAAAAAAABJBbHJlYWR5SW5pdGlhbGl6ZWQAAAAAAAEAAAAAAAAADk5vdEluaXRpYWxpemVkAAAAAAACAAAAAAAAAAxVbmF1dGhvcml6ZWQAAAADAAAAAAAAABBJbnZhbGlkU2lnbmVyU2V0AAAABAAAAAAAAAAQUHJvcG9zYWxOb3RGb3VuZAAAAAUAAAAAAAAADEFscmVhZHlWb3RlZAAAAAYAAAAAAAAAEFByb3Bvc2FsUmVqZWN0ZWQAAAAHAAAAAAAAABNUaHJlc2hvbGROb3RSZWFjaGVkAAAAAAgAAAAAAAAAD0FscmVhZHlFeGVjdXRlZAAAAAAJAAAAAAAAAA1JbnZhbGlkQWN0aW9uAAAAAAAACgAAAAAAAAAOVGltZWxvY2tBY3RpdmUAAAAAAAsAAAAAAAAAD05vUGVuZGluZ1VwZGF0ZQAAAAAM",
        "AAAAAgAAAAAAAAAAAAAADlByb3Bvc2FsQWN0aW9uAAAAAAACAAAAAQAAAAAAAAAMQ29udHJhY3RDYWxsAAAAAQAAB9AAAAASQ29udHJhY3RDYWxsQWN0aW9uAAAAAAABAAAAAAAAAA1VcGRhdGVTaWduZXJzAAAAAAAAAQAAB9AAAAATVXBkYXRlU2lnbmVyc0FjdGlvbgA=",
        "AAAAAQAAAJlBIGNhbGwgcHJvcG9zYWwgdGFyZ2V0cyBhIGNvbnRyYWN0IG1ldGhvZCB3aGljaCBtdXN0IGF1dGhvcml6ZSB0aGlzIG11bHRpc2lnCmNvbnRyYWN0IGFzIGl0cyBwcml2aWxlZ2VkIGFjdG9yLiBTaWduZXIgdXBkYXRlcyB1c2UgdGhlIHNhbWUgYXBwcm92YWwgZmxvdy4AAAAAAAAAAAAAEkNvbnRyYWN0Q2FsbEFjdGlvbgAAAAAABAAAAAAAAAAEYXJncwAAA+oAAAAAAAAAAAAAAAhmdW5jdGlvbgAAABEAAAAAAAAABGtpbmQAAAfQAAAACkFjdGlvbktpbmQAAAAAAAAAAAAGdGFyZ2V0AAAAAAAT",
        "AAAAAQAAAAAAAAAAAAAAE1VwZGF0ZVNpZ25lcnNBY3Rpb24AAAAAAgAAAAAAAAAHc2lnbmVycwAAAAPqAAAAEwAAAAAAAAAJdGhyZXNob2xkAAAAAAAABA==",
        "AAAAAAAAAAAAAAAXY2FuY2VsX3RocmVzaG9sZF91cGRhdGUAAAAAAQAAAAAAAAADd2hvAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1NdWx0aXNpZ0Vycm9yAAAA",
        "AAAAAAAAAAAAAAAYZXhlY3V0ZV90aHJlc2hvbGRfdXBkYXRlAAAAAQAAAAAAAAADd2hvAAAAABMAAAABAAAD6QAAA+0AAAAAAAAH0AAAAA1NdWx0aXNpZ0Vycm9yAAAA",
        "AAAAAAAAAAAAAAAYcHJvcG9zZV90aHJlc2hvbGRfdXBkYXRlAAAAAgAAAAAAAAADd2hvAAAAABMAAAAAAAAAA25ldwAAAAAEAAAAAQAAA+kAAAPtAAAAAAAAB9AAAAANTXVsdGlzaWdFcnJvcgAAAA==",
        "AAAAAAAAAAAAAAAcZ2V0X3BlbmRpbmdfdGhyZXNob2xkX3VwZGF0ZQAAAAAAAAABAAAD6QAAA+0AAAACAAAABAAAAAYAAAfQAAAADU11bHRpc2lnRXJyb3IAAAA=" ]),
      options
    )
  }
  public readonly fromJSON = {
    initialize: this.txFromJSON<Result<void>>,
        get_signers: this.txFromJSON<Result<Array<string>>>,
        sign_action: this.txFromJSON<Result<void>>,
        get_proposal: this.txFromJSON<Result<Proposal>>,
        get_threshold: this.txFromJSON<Result<u32>>,
        execute_action: this.txFromJSON<Result<void>>,
        propose_action: this.txFromJSON<Result<u32>>,
        reject_proposal: this.txFromJSON<Result<void>>,
        cancel_threshold_update: this.txFromJSON<Result<void>>,
        execute_threshold_update: this.txFromJSON<Result<void>>,
        propose_threshold_update: this.txFromJSON<Result<void>>,
        get_pending_threshold_update: this.txFromJSON<Result<readonly [u32, u64]>>
  }
}