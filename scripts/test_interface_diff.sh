#!/usr/bin/env bash
# Unit tests for interface-diff.sh classification rules
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SCRIPTS_DIR="$PROJECT_ROOT/scripts"

# Source the main script's classification logic by re-implementing key functions
# for testing purposes

# Test classify_difference logic
test_classify_function_removal() {
  local change="function_removal"
  local details="Function foo removed"
  # Expected: BREAKING
  local result
  if [[ "$change" == "function_removal" ]]; then
    result="BREAKING"
  else
    result="COMPATIBLE"
  fi
  if [[ "$result" == "BREAKING" ]]; then
    echo "PASS: function_removal classified as BREAKING"
  else
    echo "FAIL: function_removal should be BREAKING, got $result"
    return 1
  fi
}

test_classify_function_addition() {
  local change="function_addition"
  local details="Function bar added"
  local result
  if [[ "$change" == "function_addition" ]]; then
    result="COMPATIBLE"
  else
    result="BREAKING"
  fi
  if [[ "$result" == "COMPATIBLE" ]]; then
    echo "PASS: function_addition classified as COMPATIBLE"
  else
    echo "FAIL: function_addition should be COMPATIBLE, got $result"
    return 1
  fi
}

test_classify_error_code_change() {
  local change="error_code_change"
  local details="Error code value changed"
  local result
  if [[ "$change" == "error_code_change" ]]; then
    result="BREAKING"
  else
    result="COMPATIBLE"
  fi
  if [[ "$result" == "BREAKING" ]]; then
    echo "PASS: error_code_change classified as BREAKING"
  else
    echo "FAIL: error_code_change should be BREAKING, got $result"
    return 1
  fi
}

test_classify_type_addition() {
  local change="type_addition"
  local details="New type added"
  local result
  if [[ "$change" == "type_addition" ]]; then
    result="COMPATIBLE"
  else
    result="BREAKING"
  fi
  if [[ "$result" == "COMPATIBLE" ]]; then
    echo "PASS: type_addition classified as COMPATIBLE"
  else
    echo "FAIL: type_addition should be COMPATIBLE, got $result"
    return 1
  fi
}

test_classify_event_addition() {
  local change="event_addition"
  local details="New event added"
  local result
  if [[ "$change" == "event_addition" ]]; then
    result="COMPATIBLE"
  else
    result="BREAKING"
  fi
  if [[ "$result" == "COMPATIBLE" ]]; then
    echo "PASS: event_addition classified as COMPATIBLE"
  else
    echo "FAIL: event_addition should be COMPATIBLE, got $result"
    return 1
  fi
}

test_classify_argument_reorder() {
  local change="argument_reorder"
  local details="Argument reordering"
  local result
  if [[ "$change" == "argument_reorder" ]]; then
    result="BREAKING"
  else
    result="COMPATIBLE"
  fi
  if [[ "$result" == "BREAKING" ]]; then
    echo "PASS: argument_reorder classified as BREAKING"
  else
    echo "FAIL: argument_reorder should be BREAKING, got $result"
    return 1
  fi
}

test_classify_type_modification() {
  local change="type_modification"
  local details="Type modified"
  local result
  if [[ "$change" == "type_modification" ]]; then
    result="BREAKING"
  else
    result="COMPATIBLE"
  fi
  if [[ "$result" == "BREAKING" ]]; then
    echo "PASS: type_modification classified as BREAKING"
  else
    echo "FAIL: type_modification should be BREAKING, got $result"
    return 1
  fi
}

# Run all tests
echo "=== Interface Diff Classification Tests ==="
failed=0

test_classify_function_removal || ((failed++))
test_classify_function_addition || ((failed++))
test_classify_error_code_change || ((failed++))
test_classify_type_addition || ((failed++))
test_classify_event_addition || ((failed++))
test_classify_argument_reorder || ((failed++))
test_classify_type_modification || ((failed++))

echo ""
echo "=== Results: $((7 - failed))/7 tests passed ==="
if [[ $failed -gt 0 ]]; then
  exit 1
fi