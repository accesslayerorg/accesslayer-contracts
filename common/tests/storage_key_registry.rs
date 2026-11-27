//! Storage key registry checks: collisions, golden encodings and raw-key lint.
//!
//! Regenerate the golden file after adding keys with:
//! `UPDATE_STORAGE_KEY_GOLDEN=1 cargo test -p common --test storage_key_registry`

use common::keys::{self, RegisteredKey, StorageClass};
use soroban_sdk::xdr::ToXdr;
#[allow(unused_imports)]
use soroban_sdk::{Address, Bytes, BytesN, Env, String, Symbol};

/// Fixed contract addresses so encodings are deterministic.
fn addr(env: &Env, n: u8) -> Address {
    let strkey = match n {
        1 => "CAAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQC526",
        2 => "CABAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAEAQCAIBAFNSZ",
        _ => "CABQGAYDAMBQGAYDAMBQGAYDAMBQGAYDAMBQGAYDAMBQGAYDAMBQGCK3",
    };
    Address::from_str(env, strkey)
}

/// One encoded sample key.
pub struct Entry {
    pub contract: &'static str,
    pub enum_name: &'static str,
    pub variant: &'static str,
    pub class: StorageClass,
    pub hex: std::string::String,
}

fn encode<K>(
    env: &Env,
    enum_name: &'static str,
    keys: std::vec::Vec<K>,
    out: &mut std::vec::Vec<Entry>,
) where
    K: RegisteredKey + Clone + soroban_sdk::IntoVal<Env, soroban_sdk::Val>,
{
    let mut covered: std::vec::Vec<&'static str> = std::vec::Vec::new();
    for key in keys {
        let name = key.variant_name();
        covered.push(name);
        let val: soroban_sdk::Val = key.clone().into_val(env);
        let bytes = val.to_xdr(env);
        let mut hex = std::string::String::new();
        for b in bytes.iter() {
            hex.push_str(&std::format!("{:02x}", b));
        }
        out.push(Entry {
            contract: K::CONTRACT,
            enum_name,
            variant: name,
            class: key.storage_class(),
            hex,
        });
    }
    for (name, _) in K::VARIANTS {
        assert!(
            covered.contains(name),
            "{}::{} ({}) has no sample in storage_key_registry.rs; add one",
            enum_name,
            name,
            K::CONTRACT
        );
    }
}

/// A sample of every variant of every registered key enum.
pub fn all_entries(env: &Env) -> std::vec::Vec<Entry> {
    let mut out = std::vec::Vec::new();

    encode(
        env,
        "DataKey",
        std::vec![keys::guard::DataKey::Locked,],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::auto_invest_registry::DataKey::NextRuleId,
            keys::auto_invest_registry::DataKey::Rule(1u64),
            keys::auto_invest_registry::DataKey::WalletRules(addr(env, 1)),
        ],
        &mut out,
    );
    encode(
        env,
        "Key",
        std::vec![keys::batch_trade_contract::Key::Market(addr(env, 1)),],
        &mut out,
    );
    encode(
        env,
        "Key",
        std::vec![
            keys::fee_discount_contract::Key::Admin,
            keys::fee_discount_contract::Key::Tiers,
            keys::fee_discount_contract::Key::Volume(addr(env, 1)),
        ],
        &mut out,
    );
    encode(
        env,
        "Key",
        std::vec![
            keys::holder_insurance_pool::Key::Admin,
            keys::holder_insurance_pool::Key::Pool,
            keys::holder_insurance_pool::Key::NextId,
            keys::holder_insurance_pool::Key::Cover(1u64),
            keys::holder_insurance_pool::Key::KeyTotal(addr(env, 1)),
            keys::holder_insurance_pool::Key::Trigger(addr(env, 1)),
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::key_analytics::DataKey::CreatorKeys,
            keys::key_analytics::DataKey::Snapshot(addr(env, 1), 2u64),
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::key_staking_contract::DataKey::Treasury,
            keys::key_staking_contract::DataKey::Genesis,
            keys::key_staking_contract::DataKey::EpochDuration,
            keys::key_staking_contract::DataKey::NextStakeId,
            keys::key_staking_contract::DataKey::Stake(1u64),
            keys::key_staking_contract::DataKey::Unbonding(1u64),
            keys::key_staking_contract::DataKey::Epoch(1u32),
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::launch_auction::DataKey::AuctionCount,
            keys::launch_auction::DataKey::Auction(1u32),
            keys::launch_auction::DataKey::AuctionBids(1u32),
            keys::launch_auction::DataKey::Escrow(1u32, addr(env, 2)),
            keys::launch_auction::DataKey::Allocation(1u32, addr(env, 2)),
            keys::launch_auction::DataKey::ClearingPrice(1u32),
            keys::launch_auction::DataKey::BondingCurveOpen(1u32),
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::lp_reward_contract::DataKey::NextLpId,
            keys::lp_reward_contract::DataKey::Position(1u64),
            keys::lp_reward_contract::DataKey::Pool(addr(env, 1)),
            keys::lp_reward_contract::DataKey::TotalPositions,
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::multisig_admin::DataKey::Signers,
            keys::multisig_admin::DataKey::Threshold,
            keys::multisig_admin::DataKey::NextProposalId,
            keys::multisig_admin::DataKey::Proposal(1u32),
            keys::multisig_admin::DataKey::Approval(1u32, addr(env, 2)),
            keys::multisig_admin::DataKey::Rejection(1u32, addr(env, 2)),
            keys::multisig_admin::DataKey::Pending,
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![keys::nonce_registry::DataKey::UsedNonce(addr(env, 1), 2u64),],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::personhood_hook::DataKey::Admin,
            keys::personhood_hook::DataKey::PersonhoodProvider,
            keys::personhood_hook::DataKey::WalletToPerson(addr(env, 1)),
            keys::personhood_hook::DataKey::PersonToWallet(BytesN::from_array(env, &[1u8; 32])),
            keys::personhood_hook::DataKey::PersonWalletCount(BytesN::from_array(env, &[1u8; 32])),
            keys::personhood_hook::DataKey::MaxWalletsPerPerson,
            keys::personhood_hook::DataKey::UnlinkCooldown,
            keys::personhood_hook::DataKey::LastUnlink(addr(env, 1)),
            keys::personhood_hook::DataKey::KeyRequiresPersonhood(addr(env, 1)),
            keys::personhood_hook::DataKey::PersonPurchases(
                BytesN::from_array(env, &[1u8; 32]),
                addr(env, 2)
            ),
            keys::personhood_hook::DataKey::UnverifiedCap(addr(env, 1)),
        ],
        &mut out,
    );
    encode(
        env,
        "Key",
        std::vec![
            keys::safe_mode_contract::Key::Admin,
            keys::safe_mode_contract::Key::Mode,
            keys::safe_mode_contract::Key::PendingNormal(1u32),
        ],
        &mut out,
    );
    encode(
        env,
        "Key",
        std::vec![
            keys::sealed_launch_auction::Key::Admin,
            keys::sealed_launch_auction::Key::Next,
            keys::sealed_launch_auction::Key::Auction(1u32),
            keys::sealed_launch_auction::Key::Bids(1u32),
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::seasonal_rewards::DataKey::Admin,
            keys::seasonal_rewards::DataKey::NextCampaignId,
            keys::seasonal_rewards::DataKey::Campaign(1u64),
            keys::seasonal_rewards::DataKey::Qualification(1u64, addr(env, 2)),
            keys::seasonal_rewards::DataKey::Claimed(1u64, addr(env, 2)),
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![keys::session_nonce::DataKey::Nonce(addr(env, 1)),],
        &mut out,
    );
    encode(
        env,
        "Key",
        std::vec![
            keys::social_metadata_contract::Key::Admin,
            keys::social_metadata_contract::Key::Creator(addr(env, 1)),
            keys::social_metadata_contract::Key::Metadata(addr(env, 1)),
        ],
        &mut out,
    );
    encode(
        env,
        "Key",
        std::vec![
            keys::sponsored_transactions::Key::Admin,
            keys::sponsored_transactions::Key::Nonce(addr(env, 1), 2u64),
        ],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![keys::subscription_contract::DataKey::Subscription(
            addr(env, 1),
            2u64
        ),],
        &mut out,
    );
    encode(
        env,
        "DataKey",
        std::vec![
            keys::treasury_diversification::DataKey::Governor,
            keys::treasury_diversification::DataKey::Keeper,
            keys::treasury_diversification::DataKey::Targets,
            keys::treasury_diversification::DataKey::Routes,
            keys::treasury_diversification::DataKey::Caps,
            keys::treasury_diversification::DataKey::SwapsPaused,
            keys::treasury_diversification::DataKey::PeriodVolume(1u64),
            keys::treasury_diversification::DataKey::TimelockDelay,
            keys::treasury_diversification::DataKey::PendingTargets,
            keys::treasury_diversification::DataKey::PendingTargetsTime,
        ],
        &mut out,
    );
    encode(
        env,
        "Key",
        std::vec![
            keys::vote_escrow_contract::Key::NextId,
            keys::vote_escrow_contract::Key::Lock(1u64),
            keys::vote_escrow_contract::Key::HolderLock(addr(env, 1), 2u64),
        ],
        &mut out,
    );
    out
}

const GOLDEN_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/storage_keys.golden");

fn golden_lines(entries: &[Entry]) -> std::vec::Vec<std::string::String> {
    entries
        .iter()
        .map(|e| {
            std::format!(
                "{} {}::{} {} {}",
                e.contract,
                e.enum_name,
                e.variant,
                e.class.as_str(),
                e.hex
            )
        })
        .collect()
}

#[test]
fn every_registered_key_encodes_uniquely_within_its_contract() {
    let env = Env::default();
    let entries = all_entries(&env);
    let mut seen: std::collections::BTreeMap<(&str, &str), std::string::String> =
        std::collections::BTreeMap::new();
    for e in &entries {
        let label = std::format!("{}::{}", e.enum_name, e.variant);
        if let Some(previous) = seen.insert((e.contract, e.hex.as_str()), label.clone()) {
            panic!(
                "storage key collision in {}: {} and {} encode to the same key",
                e.contract, previous, label
            );
        }
    }
}

#[test]
fn registered_key_encodings_match_golden_file() {
    let env = Env::default();
    let actual = golden_lines(&all_entries(&env));
    if std::env::var_os("UPDATE_STORAGE_KEY_GOLDEN").is_some() {
        let mut body = std::string::String::from(
            "# contract enum::variant storage-class xdr-hex\n\
             # Generated by common/tests/storage_key_registry.rs. Existing lines must never change.\n",
        );
        for line in &actual {
            body.push_str(line);
            body.push('\n');
        }
        std::fs::write(GOLDEN_PATH, body).expect("write golden file");
        return;
    }
    let golden = std::fs::read_to_string(GOLDEN_PATH).expect("read golden file");
    let expected: std::vec::Vec<&str> = golden
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .collect();
    for line in &expected {
        let id: std::vec::Vec<&str> = line.splitn(3, ' ').take(2).collect();
        let current = actual
            .iter()
            .find(|a| a.splitn(3, ' ').take(2).eq(id.iter().copied()));
        match current {
            None => panic!(
                "storage key `{}` was removed or renamed; existing keys must keep their encoding",
                id.join(" ")
            ),
            Some(current) => assert_eq!(
                current, line,
                "storage key encoding or storage class changed; existing keys must keep their layout"
            ),
        }
    }
    for line in &actual {
        assert!(
            expected.contains(&line.as_str()),
            "new storage key `{}` is missing from the golden file; run \
             `UPDATE_STORAGE_KEY_GOLDEN=1 cargo test -p common --test storage_key_registry` \
             and commit common/tests/storage_keys.golden",
            line
        );
    }
}

#[test]
fn storage_class_is_declared_for_every_variant() {
    // `storage_keys!` requires a class per variant at compile time; this checks
    // the recorded table agrees with the per-key value.
    let env = Env::default();
    for e in all_entries(&env) {
        assert!(matches!(
            e.class,
            StorageClass::Instance | StorageClass::Persistent | StorageClass::Temporary
        ));
    }
}

/// Crates whose storage keys are not in the registry yet, with the reason.
/// Remove an entry when its keys are migrated.
const NOT_YET_MIGRATED: &[(&str, &str)] = &[
    (
        "creator-keys",
        "does not compile on main; migrate once it builds",
    ),
    (
        "creator-keys-factory",
        "depends on creator-keys, which does not compile on main",
    ),
    (
        "dividend-contract",
        "depends on creator-keys, which does not compile on main",
    ),
    (
        "creator-penalty",
        "does not compile on main; migrate once it builds",
    ),
    (
        "storage-cleanup",
        "does not compile on main; migrate once it builds",
    ),
    (
        "key-health-score",
        "key fields use the contract-local `Feature` type",
    ),
    (
        "key-liquidity-depth",
        "key fields use the contract-local `OrderSide` type",
    ),
    (
        "referral-contract",
        "key fields use the contract-local `ReferralTier` type",
    ),
    ("test-utils", "mock contracts used only by tests"),
];

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn workspace_members() -> std::vec::Vec<std::string::String> {
    let manifest =
        std::fs::read_to_string(workspace_root().join("Cargo.toml")).expect("Cargo.toml");
    let start = manifest.find("members").expect("members");
    let list = &manifest[start..manifest[start..].find(']').map(|i| start + i).expect("]")];
    let mut members: std::vec::Vec<std::string::String> = list
        .split('"')
        .skip(1)
        .step_by(2)
        .map(std::string::String::from)
        .collect();
    members.sort();
    members.dedup();
    members
}

fn rust_sources(dir: &std::path::Path, out: &mut std::vec::Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name != "tests" && name != "test_snapshots" {
                rust_sources(&path, out);
            }
        } else if name.ends_with(".rs") && !name.starts_with("test") {
            out.push(path);
        }
    }
}

/// Drops `#[cfg(test)]` modules, which may build raw keys for fixtures.
fn without_test_modules(src: &str) -> std::string::String {
    let mut out = std::string::String::new();
    let mut rest = src;
    while let Some(at) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(open) = after.find('{') else {
            rest = "";
            break;
        };
        let mut depth = 0usize;
        let mut end = after.len();
        for (i, c) in after[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

/// Returns `(line, message)` for every raw storage key or local key enum in `src`.
pub fn raw_key_violations(src: &str) -> std::vec::Vec<(usize, std::string::String)> {
    let src = without_test_modules(src);
    let compact: std::string::String = src
        .split_whitespace()
        .collect::<std::vec::Vec<_>>()
        .join(" ");
    let mut found = std::vec::Vec::new();
    let line_of = |needle: &str| {
        src.lines()
            .position(|l| l.contains(needle.trim()))
            .map(|i| i + 1)
            .unwrap_or(0)
    };
    for class in ["instance", "persistent", "temporary"] {
        let marker = std::format!(".{}() .", class);
        let marker_tight = std::format!(".{}().", class);
        let text = compact.replace(&marker, &marker_tight);
        let mut from = 0;
        while let Some(at) = text[from..].find(&marker_tight) {
            let call = &text[from + at + marker_tight.len()..];
            from += at + marker_tight.len();
            let Some(paren) = call.find('(') else {
                continue;
            };
            let method = &call[..paren];
            let method = method.split("::<").next().unwrap_or(method);
            if ![
                "set",
                "get",
                "has",
                "remove",
                "extend_ttl",
                "update",
                "try_update",
            ]
            .contains(&method)
            {
                continue;
            }
            if class == "instance" && method == "extend_ttl" {
                continue;
            }
            let arg = call[paren + 1..]
                .trim_start()
                .trim_start_matches('&')
                .trim_start();
            let raw = arg.starts_with('(')
                || arg.starts_with("symbol_short!")
                || arg.starts_with("Symbol::")
                || arg
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .next()
                    .map(|id| {
                        id.len() > 1
                            && id
                                .chars()
                                .all(|c| c.is_ascii_uppercase() || c == '_' || c.is_ascii_digit())
                    })
                    .unwrap_or(false);
            if raw {
                let snippet: std::string::String = arg.chars().take(40).collect();
                found.push((
                    line_of(&snippet[..snippet.len().min(12)]),
                    std::format!("raw storage key `{}` in {}().{}", snippet, class, method),
                ));
            }
        }
    }
    let lines: std::vec::Vec<&str> = src.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        let decl = t.strip_prefix("pub ").unwrap_or(t);
        let decl = if decl.starts_with("pub(") {
            decl.split_once(") ").map(|(_, d)| d).unwrap_or(decl)
        } else {
            decl
        };
        if let Some(name) = decl.strip_prefix("enum ") {
            let name = name
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("");
            let attrs = lines[i.saturating_sub(4)..i].join("\n");
            if name.ends_with("Key") && attrs.contains("contracttype") {
                found.push((
                    i + 1,
                    std::format!(
                        "storage key enum `{}` declared outside common/src/keys.rs",
                        name
                    ),
                ));
            }
        }
    }
    found
}

#[test]
fn contracts_do_not_use_raw_storage_keys_outside_the_registry() {
    let root = workspace_root();
    let mut violations = std::vec::Vec::new();
    for member in workspace_members() {
        if NOT_YET_MIGRATED.iter().any(|(c, _)| *c == member) {
            continue;
        }
        let mut files = std::vec::Vec::new();
        rust_sources(&root.join(&member).join("src"), &mut files);
        for file in files {
            if file.ends_with("common/src/keys.rs") {
                continue;
            }
            let src = std::fs::read_to_string(&file).expect("read source");
            for (line, msg) in raw_key_violations(&src) {
                violations.push(std::format!(
                    "{}:{}: {}",
                    file.strip_prefix(&root).unwrap_or(&file).display(),
                    line,
                    msg
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "storage keys must come from common::keys (see docs/storage-key-registry.md):\n{}",
        violations.join("\n")
    );
}

#[test]
fn lint_flags_raw_tuple_symbol_and_local_enum_keys() {
    let tuple = "fn f(env: Env, a: Address) { env.storage().persistent().set(&(symbol_short!(\"bal\"), a), &1u32); }";
    let symbol = "fn f(env: Env) { env.storage()\n    .instance()\n    .get::<_, u32>(&symbol_short!(\"admin\")); }";
    let constant = "const ADMIN: Symbol = symbol_short!(\"admin\");\nfn f(env: Env) { env.storage().instance().has(&ADMIN); }";
    let local_enum = "#[contracttype]\n#[derive(Clone)]\npub enum DataKey { Admin }";
    let registered = "fn f(env: Env) { env.storage().instance().set(&DataKey::Admin, &1u32); env.storage().instance().extend_ttl(TTL_THRESHOLD, TTL_EXTEND); }";
    let test_only = "#[cfg(test)]\nmod tests { fn f(env: Env) { env.storage().temporary().set(&(1u32, 2u32), &0u32); } }";
    assert_eq!(raw_key_violations(tuple).len(), 1);
    assert_eq!(raw_key_violations(symbol).len(), 1);
    assert_eq!(raw_key_violations(constant).len(), 1);
    assert_eq!(raw_key_violations(local_enum).len(), 1);
    assert!(raw_key_violations(registered).is_empty());
    assert!(raw_key_violations(test_only).is_empty());
}

#[test]
fn registry_table_matches_per_key_storage_class() {
    let env = Env::default();
    let key = keys::guard::DataKey::Locked;
    assert_eq!(key.storage_class(), StorageClass::Temporary);
    assert_eq!(
        keys::guard::DataKey::VARIANTS,
        &[("Locked", StorageClass::Temporary)]
    );
    assert!(!all_entries(&env).is_empty());
}
