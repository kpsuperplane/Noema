# Memory file formation

Rust reference: `4d29f6ba1f70a30b5959a0e460217feeeb5e8c04`.

Automatic updates now follow successful saved Chat compaction.
The separate pending-source size trigger is removed.
The manual update action remains available.

The human clarified the file rule after requesting Rust parity.
The updater must retain content in its current article until it would exceed
750 words, including the title and generated footnotes.
It must first combine related claims and remove repetition.
It must merge small child articles when the combined parent fits.
These are model instructions. Code still enforces the publication word limit.
This task did not run an update against live Memory.

The retained Rust prompt reference remains unchanged.
The exact prompt test applies only the human's requested editorial changes.

## Validation

Focused checks passed on the production changes in this task:
`CGO_ENABLED=0 go test ./internal/runtime -run 'TestPrimaryMemoryUpdateFollowsContextCompaction|TestMemory|TestAuxiliaryPrompts'`.
They verify update timing, existing Memory behavior, and complete instruction text.
The timing test uses an explicit model context limit.

Patch size: production +18/-40; tests +95/-58; generated GraphQL unchanged.
Inclusive tracked Go change: +113/-98, or 15 net lines.
One table test replaces two tests for the removed size trigger.
A read-only review found no further correction in the changed paths.

`CGO_ENABLED=0 go vet ./cmd/... ./internal/...` passed.
The broad test command ran all packages: `CGO_ENABLED=0 go test ./cmd/... ./internal/...`.
It found an unrelated temporary test that requires stdin and one stale prompt assertion.
The prompt assertion now checks the requested wording and passes its focused retry.
`CGO_ENABLED=0 go test ./internal/adapter -skip '^TestTmpPA069Validate$'` passed.
The temporary test remains unchanged. All other non-runtime packages passed.
These results cover the production changes above. Only the prompt assertion changed afterward.

The full runtime retry, `CGO_ENABLED=0 go test ./internal/runtime`, stalled.
It ended with exit code 143 without a stack trace or test result.
This retry is not recorded as passed. The earlier broad run completed runtime
and reported only the stale prompt assertion. Its other results remain valid;
the corrected assertion passed the focused retry below:
`CGO_ENABLED=0 go test ./internal/runtime -run '^TestRustRuntime_memory_instructions_reserve_space_below_the_page_word_limit$'`.

Unrelated frontend edits and PA-069 acceptance files remain outside this commit.
They include the temporary adapter test and meal-planning API script.
