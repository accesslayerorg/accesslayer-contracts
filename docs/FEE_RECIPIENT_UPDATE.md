# Safe Fee Recipient Address Update Procedure

## Overview

The protocol fee recipient address determines where protocol fees are routed after each trade. Updating this address requires care to ensure in-flight payments are not lost and the new address is valid before the change is committed.

## Update Procedure

### Step 1: Validate the New Address

Before updating the fee recipient address, verify:

1. **Address Format**: Ensure the new address is a valid Stellar address (G-address format, 56 characters)
2. **Address Control**: Confirm you control the private key for the new address
3. **Test Transaction**: Send a small test payment to the new address and verify receipt
4. **Account Status**: Verify the account exists on-chain and is not frozen or restricted

### Step 2: Check for In-Flight Trades

Before committing the update:

1. **Monitor Recent Activity**: Check for any pending or recently submitted trades
2. **Wait for Settlement**: Allow sufficient time for in-flight trades to settle (typically 1-2 ledgers, ~5-10 seconds)
3. **Verify Balances**: Confirm all expected fees have been received at the current recipient address

### Step 3: Perform the Update

Execute the update transaction:

```rust
// Example: Update protocol fee recipient
// Note: Actual implementation depends on admin functions available
set_protocol_fee_recipient(env, admin, new_recipient_address);
```

### Step 4: Verify the Update

After the update transaction confirms:

1. **Read Back**: Call `get_protocol_fee_recipient()` to verify the new address is stored
2. **Test Trade**: Execute a small test trade and verify fees route to the new address
3. **Monitor**: Watch the new recipient address for incoming fee payments

## Contract Validation

### What the Contract Validates

The contract performs the following validation on fee recipient updates:

- **Authentication**: Requires `admin.require_auth()` - only authorized admins can update
- **Storage Consistency**: Checks if the new address differs from the current one before writing
- **Address Type**: Validates that the parameter is a valid `Address` type

### What the Contract Does NOT Validate

The contract does **not** validate:

- ❌ Whether the address exists on-chain
- ❌ Whether you control the private key for the address
- ❌ Whether the account is active or frozen
- ❌ Whether the address is a contract or user account
- ❌ Whether there are in-flight trades that will route to the old address

**These validations are the responsibility of the admin performing the update.**

## Risk Window

### Critical Period

There is a risk window between the update transaction and the next trade:

- **Duration**: From when the update transaction is submitted until it confirms (typically 1 ledger, ~5 seconds)
- **Risk**: Trades submitted during this window may route fees to either the old or new address depending on timing
- **Mitigation**: Perform updates during low-activity periods when possible

### Post-Update Monitoring

After updating the fee recipient:

1. **Monitor Both Addresses**: Watch both old and new addresses for 5-10 minutes
2. **Verify Fee Routing**: Confirm new trades route fees to the new address
3. **Handle Edge Cases**: If fees arrive at the old address, manually forward them to the new address

## Rollback Procedure

If issues are detected after the update:

1. **Immediate Rollback**: Call the update function again with the previous address
2. **Verify Rollback**: Confirm the old address is restored via `get_protocol_fee_recipient()`
3. **Investigate**: Determine the root cause before attempting another update

## Best Practices

### Timing

- ✅ **Do**: Update during low-activity periods (e.g., off-peak hours)
- ✅ **Do**: Coordinate with your team and notify stakeholders
- ❌ **Don't**: Update during high-volume trading periods
- ❌ **Don't**: Update immediately before or after major announcements

### Testing

- ✅ **Do**: Test the full procedure on testnet first
- ✅ **Do**: Verify the new address with multiple small test transactions
- ✅ **Do**: Document the old and new addresses for audit purposes
- ❌ **Don't**: Skip validation steps to save time
- ❌ **Don't**: Update without a rollback plan

### Communication

- ✅ **Do**: Notify monitoring systems and indexers of the planned change
- ✅ **Do**: Update internal documentation with the new address
- ✅ **Do**: Keep a changelog of all fee recipient updates
- ❌ **Don't**: Perform updates without team coordination
- ❌ **Don't**: Forget to update off-chain systems that track the fee recipient

## Emergency Contacts

If issues arise during or after an update:

1. **Check Contract State**: Use `get_protocol_fee_recipient()` to verify current state
2. **Review Recent Trades**: Check recent transactions for unexpected fee routing
3. **Coordinate Rollback**: If necessary, execute the rollback procedure immediately
4. **Post-Mortem**: Document what went wrong and update this procedure accordingly

## Checklist

Before updating the protocol fee recipient address:

- [ ] New address validated (format, control, test transaction)
- [ ] No in-flight trades pending
- [ ] Update scheduled during low-activity period
- [ ] Team notified and coordinated
- [ ] Rollback plan documented
- [ ] Monitoring systems ready
- [ ] Test transaction prepared for post-update verification
- [ ] Old address documented for audit trail

## See Also

- Contract source: `creator-keys/src/lib.rs`
- Storage constants: `constants::storage::PROTOCOL_FEE_RECIPIENT`
- Related functions: `get_protocol_fee_recipient()`, `set_treasury_address()`
