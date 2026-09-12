# Voxloom Types

Shared types between `voxloom-backend`, `voxloom-website` and the SDKs. A path
dependency in both, so a change here is picked up without a commit or a rev
bump, and a change here can break both at once.

## The wire format is a contract

These structs are the public API. The docs, `openapi.yaml` and every SDK quote
their serde names, so a rename is a breaking change for all of them.

Tests in `src/transcribe.rs` pin the wire format deliberately:

- `MediaSource` is tagged with **`kind`**, not `type`.
- Feature, model and export-format names are asserted string by string.
- Every `Tier` round-trips through serde *and* equals its `lookup_key()`,
  because tiers are HashMap keys in `PricingData` and values in Stripe
  metadata. A mismatch there makes the pricing snapshot fail to deserialise and
  the pricing page show skeletons, with nothing in the logs to say why. That
  happened once.

If you rename a variant, update those tests, the docs, the OpenAPI spec and the
SDKs in the same change.

## Units

The most error-prone thing in this crate.

- One second of audio is `CREDITS_PER_MEDIA_SECOND` (1) credit at the base
  rate, multiplied by the model: Thread 1, Weave 2, Tapestry 4.
- Features add per-second surcharges on top.
- **Everything customer-facing is quoted in standard minutes**, meaning minutes
  on Weave. One standard minute is 120 credits.

Two formatters, not interchangeable:

| Function | Input | Use for |
| --- | --- | --- |
| `format_duration(secs)` | seconds | media length, speaking time, ETAs |
| `format_credits_as_time(credits)` | credits | a balance, a charge, a quote |

Passing seconds to the credit formatter halves every number; passing credits to
the duration formatter doubles them. Both shipped as live bugs before the
functions were split, so a test asserts the balance formatter agrees with
`credits_to_standard_minutes()`.

## Tiers and entitlements

`Tier::entitlements()` is the single source of truth, and
`Tier::get_features()` is built from it so a pricing card can never promise a
feature the backend does not grant. Do not write a card's feature list by hand.

`Tier::from(&str)` must never grant something by accident: an unrecognised tier
falls back to `Free`. Note that it normalises `+` to `plus` rather than
stripping it, because stripping turned a paid tier into the one below it.

Plan names are `Free`, `Solo`, `Studio`, `Agency`. They are deliberately not
the incumbent competitor's names, which are Creator, Creator+ and Pro.

## Claims that have to stay true

Numbers here appear in marketing copy, docs and the site's meta descriptions:

- `language_count()` is asserted to be exactly 101 by a test, with a message
  saying to update the site copy and docs as well. It is not a floor.
- `ExportFormat::all()` has 10 variants, asserted by a website test because the
  copy says "ten formats" as a word.
- Only add an `Entitlement` for a feature that actually exists. A
  `BurnedInSubtitles` entitlement was removed because nothing implemented it and
  two plans were advertising it.

## Style

- `cargo test` and `cargo clippy --all-targets` clean before a change is done.
- Doc comments explain why a value is what it is, not what the field is called.
- No `chrono::Local`; everything is UTC and RFC 3339 on the wire.
