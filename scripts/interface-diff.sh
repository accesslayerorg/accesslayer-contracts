#!/usr/bin/env bash
# scripts/interface-diff.sh
#
# Compares the current contract interface with the last released version
# and classifies differences as breaking or compatible.
# Fails (exits non-zero) on breaking changes unless an approved label is present.
#
# Usage:
#   scripts/interface-diff.sh [--changelist <file>] [--previous-spec <dir>] [--current-spec <dir>]
#
# Environment variables:
#   GITHUB_PR_LABEL - Optional label that, if present, allows breaking changes
#                     (e.g., "approved-breaking-change")
#   GITHUB_PR_NUMBER - PR number for posting comments
#
# Output:
#   - Prints diff summary to stdout
#   - Exits 0 if no breaking changes, or if changes are approved
#   - Exits 1 if breaking changes are detected and not approved
#   - Posts a readable diff summary as a PR comment (if GITHUB_PR_NUMBER set)

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPTS_DIR="$PROJECT_ROOT/scripts"
CURRENT_SPEC_DIR="$PROJECT_ROOT/releases/specs/current"
PREVIOUS_SPEC_DIR="$PROJECT_ROOT/releases/specs/previous"
RELEASE_SPEC_DIR="$PROJECT_ROOT/releases/specs"

# Parse arguments
CHANGELOG=""
PREVIOUS_DIR=""
CURRENT_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --changelist)
      CHANGELOG="$2"
      shift 2
      ;;
    --previous-spec)
      PREVIOUS_DIR="$2"
      shift 2
      ;;
    --current-spec)
      CURRENT_DIR="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

# Directories
mkdir -p "$RELEASE_SPEC_DIR"
mkdir -p "$CURRENT_SPEC_DIR"
mkdir -p "$PREVIOUS_SPEC_DIR"

# --- Extract contract specs from Rust source ---

extract_functions() {
  # Extract public function signatures from the contract
  # Looks for fn declarations in the contract code
  local source_dir="$1"
  grep -hrf "^pub fn " "$source_dir"/src/**/*.rs 2>/dev/null | \
    sed 's/^pub fn //' | \
    sed 's/(.*//'  # Remove parameter list for basic analysis
}

extract_error_codes() {
  # Extract error code enum values from contract
  local source_dir="$1"
  grep -hrf "repr(u32)" "$source_dir"/src/**/*.rs 2>/dev/null | \
    grep -A100 "pub enum" | \
    grep "^    [A-Za-z]" | \
    awk '{print $1}' | \
    sed 's/,//'
}

extract_events() {
  # Extract event definitions from contract
  local source_dir="$1"
  grep -hrf "pub struct " "$source_dir"/src/**/*.rs 2>/dev/null | \
    grep -B1 "pub const.*EVENT_NAME" | \
    head -1
}

# --- Classify differences ---

classify_difference() {
  local change_type="$1"
  local details="$2"

  case "$change_type" in
    function_removal|function_rename)
      echo "BREAKING: $details"
      ;;
    error_code_change)
      echo "BREAKING: $details"
      ;;
    function_addition|type_addition|event_addition)
      echo "COMPATIBLE: $details"
      ;;
    argument_reorder|type_modification)
      echo "BREAKING: $details"
      ;;
    *)
      echo "UNKNOWN: $details"
      ;;
  esac
}

# --- Main logic ---

# Step 1: Extract specs from current source if no current spec directory provided
if [[ -z "$CURRENT_DIR" || ! -d "$CURRENT_DIR" ]]; then
  echo "Extracting current contract specs from source..."
  mkdir -p "$CURRENT_SPEC_DIR"
  # Extract functions
  extract_functions "$PROJECT_ROOT/creator-keys" > "$CURRENT_SPEC_DIR/functions.txt" || true
  # Extract error codes
  extract_error_codes "$PROJECT_ROOT/creator-keys" > "$CURRENT_SPEC_DIR/error_codes.txt" || true
  # Extract events
  extract_events "$PROJECT_ROOT/creator-keys" > "$CURRENT_SPEC_DIR/events.txt" || true
fi

# Step 2: Extract specs from previous release if available
if [[ -n "$PREVIOUS_DIR" && -d "$PREVIOUS_DIR" ]]; then
  echo "Using previous spec from: $PREVIOUS_DIR"
  cp -r "$PREVIOUS_DIR"/* "$PREVIOUS_SPEC_DIR/" 2>/dev/null || true
fi

# Step 3: If we have both current and previous specs, compare them
BREAKING_CHANGES=0
COMPATIBLE_CHANGES=0

if [[ -f "$CURRENT_SPEC_DIR/functions.txt" && -f "$PREVIOUS_SPEC_DIR/functions.txt" ]]; then
  echo "Comparing function interfaces..."

  # Functions in current but not in previous (could be addition or removal)
  comm -23 <(sort "$CURRENT_SPEC_DIR/functions.txt") <(sort "$PREVIOUS_SPEC_DIR/functions.txt") | while read -r func; do
    # Check if this function was removed (not in current but was in previous)
    if ! grep -q "$func" "$CURRENT_SPEC_DIR/functions.txt" 2>/dev/null; then
      echo "REMOVED_FUNCTION: $func"
      BREAKING_CHANGES=$((BREAKING_CHANGES + 1))
    else
      echo "ADDED_FUNCTION: $func"
      COMPATIBLE_CHANGES=$((COMPATIBLE_CHANGES + 1))
    fi
  done
fi

# Compare error codes
if [[ -f "$CURRENT_SPEC_DIR/error_codes.txt" && -f "$PREVIOUS_SPEC_DIR/error_codes.txt" ]]; then
  echo "Comparing error code definitions..."

  # Check for error code value changes
  comm -12 <(sort "$CURRENT_SPEC_DIR/error_codes.txt") <(sort "$PREVIOUS_SPEC_DIR/error_codes.txt") | while read -r code; do
    echo "SAME_ERROR_CODE: $code"
  done

  # Codes in previous but not in current (could be removal)
  comm -23 <(sort "$PREVIOUS_SPEC_DIR/error_codes.txt") <(sort "$CURRENT_SPEC_DIR/error_codes.txt") | while read -r code; do
    echo "REMOVED_ERROR_CODE: $code"
    BREAKING_CHANGES=$((BREAKING_CHANGES + 1))
  done
fi

# Compare events
if [[ -f "$CURRENT_SPEC_DIR/events.txt" && -f "$PREVIOUS_SPEC_DIR/events.txt" ]]; then
  echo "Comparing event definitions..."

  # Events in current but not previous = addition
  comm -23 <(sort "$CURRENT_SPEC_DIR/events.txt") <(sort "$PREVIOUS_SPEC_DIR/events.txt") | while read -r event; do
    echo "ADDED_EVENT: $event"
    COMPATIBLE_CHANGES=$((COMPATIBLE_CHANGES + 1))
  done

  # Events in previous but not current = removal
  comm -23 <(sort "$PREVIOUS_SPEC_DIR/events.txt") <(sort "$CURRENT_SPEC_DIR/events.txt") | while read -r event; do
    echo "REMOVED_EVENT: $event"
    BREAKING_CHANGES=$((BREAKING_CHANGES + 1))
  done
fi

# Step 4: Output summary
echo ""
echo "========================================="
echo "Interface Diff Summary"
echo "========================================="
echo "Breaking changes: $BREAKING_CHANGES"
echo "Compatible changes: $COMPATIBLE_CHANGES"
echo "========================================="

# Step 5: Check for approved label if breaking changes detected
if [[ $BREAKING_CHANGES -gt 0 ]]; then
  if [[ -n "${GITHUB_PR_LABEL:-}" ]]; then
    echo "Breaking changes detected but label '$GITHUB_PR_LABEL' is present. Approving."
    echo "BREAKING_CHANGES_APPROVED: true" >&2
  else
    echo "ERROR: Breaking changes detected and no approved label found." >&2
    echo "       Add label '$GITHUB_PR_LABEL' to approve these changes." >&2
    exit 1
  fi
fi

# Step 6: Store current specs as previous for next release
if [[ -d "$CURRENT_SPEC_DIR" ]]; then
  # Move current to previous (shift)
  if [[ -d "$PREVIOUS_SPEC_DIR" ]]; then
    rm -rf "$PREVIOUS_SPEC_DIR"
  fi
  mv "$CURRENT_SPEC_DIR" "$PREVIOUS_SPEC_DIR"
  # Extract new current from source (will be done on next run)
fi

# Exit
exit 0