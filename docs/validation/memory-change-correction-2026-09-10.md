# Memory change correction

Base revision: `3b11ca3a`.

The user reported rejection of an unknown `expected_hash` field after correction.
The field is allowed within `upserts`, but not within `metadata_updates` or at the root.
The original failed payload was not available. Its exact field location remains unknown.

Memory now validates tool output against the schema already supplied to the model.
Schema errors identify the rejected field and its schema location.
The existing correction request includes that detail.
The change preserves the single correction attempt and publication checks.

## Validation

One regression test supplies an invalid hash within a metadata update.
It checks that the correction identifies both the field and the metadata operation.
It also checks that the rejected attempt leaves Memory and its checkpoint unchanged.
A replacement page edit uses the valid hash and publishes successfully.

- `CGO_ENABLED=0 go test ./internal/runtime -run 'TestMemory'` passed.
- `CGO_ENABLED=0 go test ./cmd/... ./internal/...` passed after updating one old error-text assertion.
- `CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed.

These checks cover the current production and test changes.
The original live update has not been repeated.
The development inspection socket reported authenticated access after the rebuild.

## Size and review

Production Go grows by 10 lines. Tests grow by 46 lines.
Generated GraphQL is unchanged. Inclusive tracked Go grows by 56 lines.
Tracked Go totals are 94,538 production, 68,223 test, and 78,987 generated lines.
The inclusive total is 241,748 lines.
The existing inclusive repository size already exceeds its historical 80% gate.
This bounded fix does not resolve that existing budget failure.

Review checked strict field rejection, valid page hashes, and publication after correction.
The change adds no persisted fields or retry attempts.
