# Emergency Pause — Operator Guide

The emergency pause is a critical safety mechanism. Misusing it can cause
harm to users — pausing without a recovery plan, or unpausing before a fix
is deployed, can leave funds locked or vulnerabilities active.

## When to Pause

Activate the emergency pause **only** when you have strong evidence of:

| Condition | Example |
|-----------|--------|
| Suspected active exploit | Unusual drain from treasury or holder balances |
| Critical bug in trade logic | Buy/sell producing wrong balances |
| Abnormal treasury drain | Treasury balance decreasing without expected trades |
| Compromised admin key | Evidence of unauthorized transactions |

> **Do not pause** for speculative or precautionary reasons without evidence.
> Pausing halts all trading for all users.

## Steps While Paused

1. **Assess impact** — Determine the scope of the issue. Check on-chain event logs
   for anomalous transactions. Identify affected users and amounts.

2. **Communicate** — Notify the community immediately via official channels
   (Discord, Telegram, Twitter/X). Be transparent about what is known and unknown.

3. **Reproduce the issue** — Confirm the root cause in a local or testnet
   environment before preparing any fix.

4. **Prepare the fix** — Develop, test, and audit the remediation. Do not rush.
   A second bug introduced during an emergency is worse than the original.

5. **Plan migration if needed** — If state needs to be migrated, prepare and
   test the migration script before unpausing.

## When It Is Safe to Unpause

Only unpause when **all** of the following are true:

- [ ] Root cause is confirmed and understood
- [ ] Fix is deployed and verified on testnet
- [ ] Fix is deployed to mainnet (or migration is complete)
- [ ] At least one independent reviewer has confirmed the fix
- [ ] Community has been notified of the timeline

> ⚠️ **Warning:** Unpausing without a deployed fix leaves the same vulnerability
> active. An attacker who triggered the original pause may attempt to exploit
> it again immediately after unpausing.

## Recovery Checklist

```
[ ] Root cause confirmed
[ ] Fix tested on testnet
[ ] Fix deployed to mainnet
[ ] Migration complete (if required)
[ ] Independent review complete
[ ] Community notified
[ ] Unpause executed
[ ] Post-mortem published within 72 hours
```

## Post-Mortem

Publish a post-mortem within 72 hours of unpausing covering:
- What happened
- How it was detected
- Impact on users
- The fix applied
- Steps taken to prevent recurrence
