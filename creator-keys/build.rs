use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(read_dir) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = read_dir
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// Counts `{` and `}` on a single line outside line comments and basic string literals.
fn brace_delta(line: &str) -> i32 {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    let mut delta = 0i32;

    while i < bytes.len() {
        let b = bytes[i];
        if in_string {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == b'"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if b == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            break;
        }
        if b == b'"' {
            in_string = true;
            i += 1;
            continue;
        }
        if b == b'{' {
            delta += 1;
        } else if b == b'}' {
            delta -= 1;
        }
        i += 1;
    }

    delta
}

/// Extracts `fn_name` from a trimmed line starting with `pub fn <fn_name>(`.
fn parse_pub_fn_name(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix("pub fn ")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        return None;
    }
    let after = rest[name.len()..].trim_start();
    if after.starts_with('(') {
        Some(name)
    } else {
        None
    }
}

fn extract_contract_entrypoints(src_dir: &Path) -> BTreeSet<String> {
    let mut files = Vec::new();
    collect_rs_files(src_dir, &mut files);

    let mut entrypoints = BTreeSet::new();

    for path in files {
        println!("cargo:rerun-if-changed={}", path.display());
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));

        let mut saw_contractimpl = false;
        let mut in_impl = false;
        let mut brace_depth = 0i32;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("#[contractimpl]") {
                saw_contractimpl = true;
                continue;
            }
            if saw_contractimpl && !in_impl {
                if trimmed.starts_with("impl CreatorKeysContract") {
                    in_impl = true;
                    brace_depth = brace_delta(line);
                    saw_contractimpl = false;
                    continue;
                } else if trimmed.starts_with("impl ") {
                    saw_contractimpl = false;
                }
            }
            if in_impl {
                if brace_depth == 1 {
                    if let Some(name) = parse_pub_fn_name(trimmed) {
                        entrypoints.insert(name);
                    }
                }
                brace_depth += brace_delta(line);
                if brace_depth <= 0 {
                    in_impl = false;
                    brace_depth = 0;
                }
            }
        }
    }

    entrypoints
}

/// Extracts all double-quoted function identifiers listed between
/// `// AUTH_MATRIX_BEGIN` and `// AUTH_MATRIX_END` in `tests/auth_matrix.rs`.
fn extract_matrix_entries(matrix_path: &Path) -> BTreeSet<String> {
    let content = fs::read_to_string(matrix_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", matrix_path.display()));
    let mut names = BTreeSet::new();
    let mut active = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "// AUTH_MATRIX_BEGIN" {
            active = true;
            continue;
        }
        if trimmed == "// AUTH_MATRIX_END" {
            active = false;
            continue;
        }
        if !active || trimmed.starts_with("//") {
            continue;
        }
        // Extract the first string literal on each matrix row (the entrypoint name).
        if let Some(start) = trimmed.find('"') {
            let rest = &trimmed[start + 1..];
            if let Some(end) = rest.find('"') {
                let candidate = &rest[..end];
                if !candidate.is_empty()
                    && candidate
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_')
                {
                    names.insert(candidate.to_string());
                }
            }
        }
    }

    names
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let matrix_path = manifest_dir.join("tests").join("auth_matrix.rs");

    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=tests/auth_matrix.rs");

    let entrypoints = extract_contract_entrypoints(&src_dir);

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let generated_path = out_dir.join("contract_entrypoints.rs");
    let mut generated = String::from(
        "/// Auto-generated by `build.rs` from every `#[contractimpl] impl CreatorKeysContract`.\n",
    );
    generated.push_str("pub const CONTRACT_ENTRYPOINTS: &[&str] = &[\n");
    for name in &entrypoints {
        generated.push_str(&format!("    \"{name}\",\n"));
    }
    generated.push_str("];\n");
    fs::write(&generated_path, generated)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", generated_path.display()));

    if matrix_path.exists() {
        let covered = extract_matrix_entries(&matrix_path);
        let missing: Vec<&String> = entrypoints.difference(&covered).collect();
        if !missing.is_empty() {
            panic!(
                "Authorization matrix check failed: {} public CreatorKeysContract entrypoint(s) \
                 are missing from creator-keys/tests/auth_matrix.rs:\n  {}\n\
                 Add each entrypoint to the matrix (see docs/security.md).",
                missing.len(),
                missing
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("\n  ")
            );
        }
    }
}
