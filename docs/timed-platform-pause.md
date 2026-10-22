# Timed platform pause

`pause_all(admin, reason, max_duration_seconds)` lets the protocol admin halt
trading immediately. The reason must contain 1–512 bytes and duration must be
between one second and seven days. An active timed halt cannot be extended by
calling this method again. A checked timestamp addition rejects overflow.

Every `swap` and the existing shared trade guards reject trading before
`resume_at` with `ContractError::PlatformPaused` (117). Soroban errors contain a
numeric code; `get_platform_pause()` returns the reason, start, resume timestamp
and effective active status for UI and incident tooling.

At `timestamp == resume_at`, the timed halt stops blocking calls without a
keeper or admin transaction. The first successful guarded call afterward
records the automatic resume and emits its event. Soroban cannot emit an event
solely because wall-clock time passed. `resume_all(admin)` permits the protocol
admin to end an active timed halt early; other callers cannot resume it.

The older protocol pause, multisig platform pause and per-key pauses remain
independent. Ending this timed halt never clears those controls, so another
active pause can continue to block a trade.

## Storage and events

The new persistent `TimedPauseKey::State` contains a `TimedPlatformPause` and
receives the standard full TTL window. It retains the last reason and
timestamps after resume. No old storage key or event schema is repurposed.

| Meaning | Topics | Payload fields |
| --- | --- | --- |
| PlatformPaused | `all_pause`, admin | reason, paused_at, resume_at, active |
| PlatformResumed | `all_resum` | reason, resume_at, timestamp, automatic |

The new error is appended after existing numeric codes; old codes keep their
values. Tests cover authorization, both swap directions, rejection of pause
extension, exact expiry, automatic and early resume, event data, and independence
from the older protocol pause.
