#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="$(cd "$script_dir/.." && pwd)"
python_bin="${PYTHON:-python3}"

cd "$root"

target="$("$python_bin" "$script_dir/budgets.py" target)"
packages_file="$(mktemp)"
trap 'rm -f "$packages_file"' EXIT
"$python_bin" "$script_dir/budgets.py" packages > "$packages_file"

package_args=()
while IFS= read -r package; do
  if [ -n "$package" ]; then
    package_args+=("-p" "$package")
  fi
done < "$packages_file"

rustup target add "$target"
cargo build --release --target "$target" "${package_args[@]}"
"$python_bin" "$script_dir/budgets.py" sizes
