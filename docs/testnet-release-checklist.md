# Contract Testnet Release Checklist

Use this checklist for any `creator-keys` update targeting Stellar testnet verification.

- Confirm the branch contains only intended contract/docs/test updates.
- Run `cargo fmt --all -- --check`.
- Run `cargo clippy --workspace --all-targets -- -D warnings`.
- Run `cargo test --workspace`.
- Build artifact: `stellar contract build --package creator-keys`.
- Verify artifact exists at `target/wasm32v1-none/release/creator_keys.wasm`.
- Record artifact metadata (at minimum path + SHA256 checksum) in PR notes.
- Deploy to testnet and capture the returned contract ID.
- Run post-deploy smoke checks (`register_creator`, then `get_creator`) against that contract ID.
- Add contract ID, artifact metadata, and smoke-test output summary to the PR.
