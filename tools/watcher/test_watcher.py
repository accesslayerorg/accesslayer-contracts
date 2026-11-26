#!/usr/bin/env python3
# Test file for contract event watcher
import sys
import os
import json
import tempfile

# Add the project to the path
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from tools.watcher.watcher import (
    load_rules,
    matches_event_pattern,
    Deduplicator,
    load_checkpoint,
    save_checkpoint,
    generate_explorer_link,
    Rule,
)


def test_load_rules():
    """Test that rules are loaded correctly from YAML."""
    rules = load_rules()
    assert len(rules) > 0, "Should load at least one rule"
    print(f"PASS: Loaded {len(rules)} rules")


def test_matches_event_pattern():
    """Test event pattern matching."""
    # Test matching
    event = {"event_name": "EscapeRedeemedEvent", "ledger": 12345, "amount": 100}
    pattern = {"event_name": "EscapeRedeemedEvent"}
    assert matches_event_pattern(event, pattern), "Should match EscapeRedeemedEvent"
    
    # Test non-matching
    event2 = {"event_name": "BuyEvent", "ledger": 12345}
    assert not matches_event_pattern(event2, pattern), "Should not match BuyEvent"
    
    # Test wildcard
    pattern_wild = {"event_name": "Event*"}
    event3 = {"event_name": "EscapeRedeemedEvent"}
    assert matches_event_pattern(event3, pattern_wild), "Should match Event* wildcard"
    
    print("PASS: Event pattern matching works correctly")


def test_deduplicator():
    """Test alert deduplication logic."""
    dedup = Deduplicator()
    
    # First alert should be allowed
    assert dedup.should_alert("key1:123", "rule1"), "First alert should be allowed"
    
    # Record it
    dedup.record_alert("key1:123", "rule1")
    
    # Same key within cooldown should be suppressed
    assert not dedup.should_alert("key1:123", "rule1"), "Same key within cooldown should be suppressed"
    
    # Different key should be allowed
    assert dedup.should_alert("key2:123", "rule1"), "Different key should be allowed"
    
    print("PASS: Deduplication logic works correctly")


def test_checkpoint():
    """Test checkpoint save/load."""
    with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
        tmp_path = f.name
    
    try:
        # Create and save checkpoint
        checkpoint = load_checkpoint()
        checkpoint.last_processed_ledger = 1000
        checkpoint.last_checkpoint_time = time.time()
        checkpoint.rule_states = {"rule1": {"count": 5}}
        
        save_checkpoint(checkpoint, tmp_path)
        
        # Load it back
        loaded = load_checkpoint(tmp_path)
        assert loaded.last_processed_ledger == 1000, "Should preserve ledger"
        assert loaded.rule_states == {"rule1": {"count": 5}}, "Should preserve rule states"
        
        print("PASS: Checkpoint save/load works correctly")
    finally:
        os.unlink(tmp_path)


def test_explorer_link():
    """Test explorer link generation."""
    link = generate_explorer_link("testnet", 12345)
    assert "testnet" in link.lower() or "stellar.expert" in link, f"Should contain testnet link, got: {link}"
    
    link2 = generate_explorer_link("public", 54321)
    assert "stellar.expert" in link2, f"Should contain explorer link, got: {link2}"
    
    print("PASS: Explorer link generation works correctly")


def test_rule_creation():
    """Test Rule dataclass creation."""
    rule = Rule(
        id="test_rule",
        name="Test Rule",
        description="A test rule",
        event_pattern={"event_name": "SomeEvent"},
        severity="high",
    )
    assert rule.id == "test_rule"
    assert rule.name == "Test Rule"
    assert rule.severity == "high"
    print("PASS: Rule creation works correctly")


if __name__ == "__main__":
    test_load_rules()
    test_matches_event_pattern()
    test_deduplicator()
    test_checkpoint()
    test_explorer_link()
    test_rule_creation()
    print("\n=== All watcher tests passed! ===")