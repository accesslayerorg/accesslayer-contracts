# Security Policy

## Reporting a vulnerability

Please do not open public GitHub issues for security vulnerabilities.

Instead:

1. Gather the affected functions, state assumptions, and impact details.
2. Contact the maintainers privately or use GitHub private vulnerability reporting if it is enabled for this repository.
3. Give the maintainers time to validate and coordinate a fix before public disclosure.

## Scope

Security reports are especially relevant for:

- authorization and signer requirements
- storage layout and upgrade assumptions
- buy and sell execution
- pricing or fee calculations
- event integrity and state consistency

## Automated checks

Every pull request runs Scout for Soroban, `cargo-deny` (advisories, licenses and
dependency sources) and `cargo-audit`, and a weekly `cargo-audit` job opens a
tracked issue for each new advisory. GitHub Actions are pinned to commit SHAs.

Reviewed findings are recorded in `.github/security/suppressions.toml` with a
justification, an owner and an expiry date. See
[docs/security-ci.md](./docs/security-ci.md) for the triage workflow and the
suppression rules.

## Responsible disclosure

- Do not share exploit details publicly before a fix exists.
- Include exact reproduction steps or a proof of concept when possible.
- Treat all contract correctness bugs as potentially high-impact until reviewed.
