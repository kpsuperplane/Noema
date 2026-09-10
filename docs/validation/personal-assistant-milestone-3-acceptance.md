# Personal Assistant Milestone 3 Acceptance

Date: 2026-08-29

Focused regression date: 2026-08-30

Milestone 3 is complete.

This record covers the 28 Tasks in Milestone 3.

## Result

The implementation supports the five shared acceptance paths.

The grouped live cases passed all 28 task paths.
The housing case now includes exact weekday 08:30 trips for all three homes.

The return packet received receipt `M3-0006`.
The corrected travel packet received receipt `M3-0007`.

The human authorized completion of the remaining controlled test infrastructure on 2026-08-29.
This authorization waives affected-user review for the controlled accessibility fixture.

## Implemented slice

| Commit | Change | User-visible result |
| --- | --- | --- |
| `763d1ddb` | Add exact Task calculations. | A Task can evaluate saved decimal expressions and return each exact step. |
| `154eef77` | Add private Task source uploads. | A human can upload private source files into Task-owned artifact storage. |
| `007a7cde` | Add raster image OCR. | File parsing can extract text from supported raster images while preserving the source. |
| `00c4b65b` | Parse email source files as text. | Task file parsing can read saved email messages. |
| `9ae8a443` | Fix calculation lint. | The calculation path follows the current Rust checks. |
| `df281a24` | Preserve export sources and check accessible HTML. | Exported HTML keeps source metadata and receives automated accessibility checks. |
| `da6689ca` | Preserve local artifact citations. | Artifact citations remain linked to the exact local source. |
| `7f62bc5f` | Teach Tasks to cite local artifacts. | Task results can cite exact artifact versions. |
| `1f070eaf` | Upload Task artifacts through the browser. | A reviewed browser action can upload an exact Task artifact. |
| `075a3760` | Route file uploads past Obscura. | Upload requests use a browser provider that supports file selection. |
| `1e2a0600` | Stop repeated browser provider switches. | A failed provider switch returns a bounded retry-later result. |
| `7cc47a47` | Preserve Task finish tool priority. | Executor and Reviewer roles keep their required finish tools first. |

The slice adds no case, option, application, inventory, or domain-specific record system.
Task files and artifacts remain the current authorities for these bounded cases.

## Post-acceptance simplification

The 2026-08-29 evidence below records the implementation that ran at that time.

Two changes replaced parts of that implementation on 2026-08-30.

| Commit | Change | Current result |
| --- | --- | --- |
| `f09efacb` | Replace the dedicated calculator with bounded Luau. | Agents can run general deterministic code over read-only JSON input. |
| `3216bb1e` | Simplify Task artifact contracts. | Uploads and exports no longer require manual source manifests or hidden HTML checks. |
| `648a20ba` | Keep artifact digests internal. | Human and agent contracts use immutable version IDs instead of content hashes. |

Artifact IDs and immutable version IDs remain available for citations and exact file selection.
SHA-256 remains an internal local-file integrity check.
It is no longer part of the new Task upload or agent-facing artifact receipts.

The focused live regression passed on 2026-08-30.
The historical results below remain evidence for the other Milestone 3 paths.

### Current replacement-path evidence

Task `task:18d07a17681adac7353` received a PDF, spreadsheet, email, and image.
The Task parsed all four files and merged duplicate records into three inventory items.

| Check | Current result |
| --- | --- |
| General calculation | `code.run_lua` calculated `79,300` cents and returned `USD 793.00`. |
| Private file intake | Four Task-owned source artifacts preserved the original bytes and immutable version IDs. |
| Mixed parsing | PDF, spreadsheet, email, and image parsing completed successfully. |
| Deduplication | The Task merged matching desk records and retained three physical items. |
| Source citations | The result cited each source artifact and the precise parser location available for each claim. |
| Artifact creation | Artifact `artifact:18d07aea10326162663` saved the final HTML document. |
| Accessibility | Manual checks passed for language, title, main landmark, heading, caption, headers, image alternatives, and link names. |
| Review | Reviewer run `run:18d07afb344139d285b` approved the result. |

The first live attempts exposed an invalid provider schema for open Lua input.
Noema now sends that open input schema without strict conversion.
Planner attempt 5 then completed through the current provider path.

### Patch size

The implementation changed Rust code against Milestone 2 commit `51f89e2f`.

| Measure | Change |
| --- | ---: |
| Production lines | `+1,299` |
| Test lines | `+343` |
| Total Rust lines | `+1,642` |
| Test declarations | `+16` |

The size check passed its limits of 1,600 production lines, 500 test lines, and 20 new tests.

## Shared acceptance evidence

### Mixed private packet

Task `task:18d05af89805675b3a5` received four private source artifacts.

| Format | Artifact | Artifact version | Source version |
| --- | --- | --- | --- |
| PDF | `artifact:18d05afff30c37c7476` | `artifact_version:18d05afff30c444d477` | `rev-2026-08-20` |
| Spreadsheet | `artifact:18d05afff794ad2c478` | `artifact_version:18d05afff794bb39479` | `workbook-4` |
| Email | `artifact:18d05afffa81cc3a47c` | `artifact_version:18d05afffa81d8f247d` | `message-8821` |
| Image | `artifact:18d05afffe6331f147e` | `artifact_version:18d05afffe63404547f` | `photo-2` |

The result deduplicated the desk across sources.
It retained the source owner and private disclosure scope.
It cited every inventory record through exact artifact links.

### Exact calculation

Task `task:18d0596a400e480d1fa` compared two job offers.
The Task used `calculation.evaluate` for the annual and monthly totals.
The saved result includes the input values, expressions, and exact step values.

Other live cases reproduced these results:

- Interview expenses: `$86.40 + $240.00 = $326.40`.
- Study time: `13` hours and `2.60` hours per entry.
- Quiz results: `390` points and `78.00` points per entry.
- Tax receipts: `$3,425.50`.
- Home-office share: `10.00%`.
- Asset value: `$793`.
- Repair difference: `$90`.
- Utility totals: `$1,479` and `$1,524`.
- Utility savings: `$177` and `$132`.
- Training and exam path: `$960`.

No accepted generated number depends only on model arithmetic.

The 2026-08-30 focused regression used `code.run_lua` instead of the removed calculator.
It reproduced the asset total as `79,300` cents and `USD 793.00`.

### Source and disclosure preservation

Task `task:18d05af89805675b3a5` exported an accessible private inventory.

| Item | Value |
| --- | --- |
| Artifact | `artifact:18d05e06e1f16232661` |
| Artifact version | `artifact_version:18d05e06e1f1831c662` |
| Filename | `private-move-inventory-accessible.html` |
| Bytes | `25,509` |
| SHA-256 | `b65d5edc888f51a6bc3ba0b3b01316cee1f1dd8e4c65e91216f7ef51196cc546` |

The artifact metadata names all four sources.
It retains each source version, owner, and disclosure scope.

### Portal upload and receipt

Task `task:18d05ed469bd6bc08eb` uploaded `cedar-labs-application-packet.html`.
The portal returned receipt `M3-0003`.

The receipt matched 7,186 bytes and SHA-256 `68b86be868a8166b243da42cedb2ea64442eb13c9e0c2bb705963c54fe4deb1d`.

Task `task:18d05eea67498fdcc20` uploaded `renewal-packet-2026-08-29.md`.
The portal returned receipt `M3-0004`.

The receipt matched 12,766 bytes and SHA-256 `432d2db4cb970ed66f81f911c9cd481937dd3fb4041eb6eb8445a546dfd1336d`.

Task `task:18d05eea8d4193d8c3a` uploaded `accessibility-campaign-packet.md`.
The portal returned receipt `M3-0005`.

The receipt matched 30,076 bytes and SHA-256 `89b20a6beda99f08017d30b7236b876bd92f02a3a08e5d0e7909f15565285b57`.
The Task saved receipt artifact `artifact:18d062b8471b7bb94b83`.

Task `task:18d05eea68226e8bc24` uploaded `ORD-447-return-and-dispute-packet.md`.
The portal returned receipt `M3-0006`.

The receipt matched 8,823 bytes and SHA-256 `13807e141cc1a05603b28c314cd9029069363806ca419dd6d449b76d9ef954dc`.
The Task saved receipt artifact `artifact:18d06581eeaa31819a5b`.

Task `task:18d05eea8649b293c2a` first produced a packet with one malformed State Department URL.
The upload and submit actions were declined before the portal changed.

The Task received a natural human correction request through normal cancellation and reopen controls.
It created corrected artifact `artifact:18d065d01e4f22e0a32c`.
Its version is `artifact_version:18d065d01e4f3165a32d`.

The corrected packet is `spain-readiness-and-claim-packet-corrected.md`.
The portal returned receipt `M3-0007`.

The receipt matched 13,886 bytes and SHA-256 `3930a306a65cc73a33514dbbd3e07c501f2fd1e806c06f2d600bbc00c9ee7311`.
The portal records contain one submission for the corrected Spain packet.

Task `task:18d05e1a1f272e1d87f` tested an unknown browser outcome.
The portal accepted the exact file before the browser displayed a Cloudflare 502 response.

Noema reopened the submission records.
It found receipt `M3-0001` with the expected filename, bytes, and hash.
It did not submit the form again.

The stored browser action did not use the typed unknown-outcome state in this fixture.
A focused unit test separately covers that typed recovery state.

### Accessible export

Automated checks passed for the private inventory HTML.

- The document declares its language.
- The document has a non-empty title.
- The document contains one main landmark.
- The document contains one first-level heading.
- Every image has alternative text.
- The table has structured headers and a caption.
- Every link has an accessible name.

The human waived affected-user review because this artifact belongs to controlled test infrastructure.
This waiver does not prove usability for an affected user.

The 2026-08-30 replacement artifact also passed deliverable-specific manual checks.
These checks covered the same semantic document requirements without a hidden global validator.

## Task evidence

| Task | Live evidence | Accepted result | Remaining boundary |
| ---: | --- | --- | --- |
| 23 | `task:18d05ed469bd6bc08eb` | Built a cited application packet from several source records. | Connected source intake remains separate. |
| 25 | `task:18d05ed469bd6bc08eb` | Matched two receipts and charges. The exact total was `$326.40`. | No employer expense system changed. |
| 27 | `task:18d05ed469bd6bc08eb`; `task:18d05eea67498fdcc20` | Produced current PMP obligations and one receipt-backed upload. | No fee or renewal was submitted. |
| 29 | `task:18d05ed469bd6bc08eb` | Produced truthful application materials and uploaded the exact artifact. | No employment application was submitted. |
| 30 | `task:18d0596a400e480d1fa` | Compared two offers with reproducible annual and monthly totals. | Live benefit and tax connections remain separate. |
| 34 | `task:18d05ed46b18ff708ef` | Produced a three-source peer-reviewed evidence map. | A scholarly index is not required for this bounded case. |
| 36 | `task:18d05af89805675b3a5` | Parsed PDF, spreadsheet, email, and image sources into one cited inventory. | More complex layout OCR needs later cases. |
| 37 | `task:18d05ed46b18ff708ef` | Audited a study log with exact totals and an accessible text chart. | Complex statistics need a different production path. |
| 40 | `task:18d05ed46b18ff708ef` | Compared three current programs with eligibility, cost, dates, and unknowns. | Enrollment was outside the request. |
| 42 | `task:18d05ed4875a68788f5` | Built a tax-readiness packet and exposed the missing W-2. | A qualified accountant owns tax conclusions. |
| 44 | `task:18d05ed4875a68788f5` | Normalized supplied policy limits and household changes. | A licensed professional owns coverage conclusions. |
| 49 | `task:18d05eea67498fdcc20` | Built current PMP and passport plans with exact dates and a portal receipt. | Fees and official renewal remain unsubmitted. |
| 50 | `task:18d05eea68226e8bc24` | Built a source-preserving consumer dispute packet and saved receipt `M3-0006`. | No merchant contact occurred. |
| 55 | `task:18d05ed48d5ad17b8fe` | Built a dated appointment brief from private evidence. | A clinician owns diagnosis and treatment. |
| 58 | `task:18d05eea8c1e2ad8c33` | Compared three accessible physical-therapy providers with current public evidence. | Exact network status and available slots remain unknown. |
| 64 | `task:18d05ed48d5ad17b8fe` | Preserved conflicting advice in a cited treatment decision brief. | The brief stops at qualified clinical review. |
| 65 | `task:18d05ed4ab303da7905` | Deduplicated asset records and calculated the exact inventory value. | Live recall feeds remain separate. |
| 67 | `task:18d05ed4ab303da7905` | Compared two repair bids and exposed missing insurance proof. | No vendor was contacted or hired. |
| 68 | `task:18d05ed4ab303da7905` | Reproduced first-year utility totals and savings. | No service change occurred. |
| 72 | `task:18d05eea68226e8bc24` | Built a return packet with exact refund, later verification rules, and receipt `M3-0006`. | No shipping or refund occurred. |
| 78 | `task:18d05eea8649b293c2a` | Corrected the malformed source link and uploaded the current Spain readiness packet under receipt `M3-0007`. | No travel purchase occurred. |
| 79 | `task:18d05eea8649b293c2a` | Kept voucher, claim, receipts, and headroom separate under receipt `M3-0007`. | No claim was filed. |
| 83 | `task:18d05eea8c1e2ad8c33` | Checked all three homes with dated 08:30 trips. The exact totals were `23:07`, `23:07`, and `39:07`. | Real-time delays and complete step-free walking routes remain unknown. |
| 84 | `task:18d05ed4ab303da7905` | Tracked a deposit claim and reconciled move records. | No payment or claim action occurred. |
| 86 | `task:18d05ed4ab303da7905` | Deduplicated three school notices while retaining all dates. | Connected school intake remains separate. |
| 89 | `task:18d05eea8d4193d8c3a` | Built a current, truthful scholarship campaign packet and saved receipt `M3-0005`. | No application or recommendation request was sent. |
| 91 | `task:18d05eea8d4193d8c3a` | Found a current remote volunteer role, built a bounded schedule, and saved receipt `M3-0005`. | No volunteer commitment was sent. |
| 100 | `task:18d05af89805675b3a5` | Exported accessible HTML and passed automated checks. | The human waived affected-user review for this controlled fixture. |

## Provider-neutral behavior

These cases describe source and action behavior.
They do not depend on Gmail, Calendar, Notion, or another named service.

| Behavior | Current evidence |
| --- | --- |
| File intake | Task-owned private artifacts preserve source bytes and metadata. |
| File parsing | PDF, spreadsheet, email, and raster image sources produced bounded text. |
| Calculation | Saved decimal expressions returned exact intermediate values. |
| Research | Current public sources retained URLs, dates, limits, and uncertainty. |
| Route | The housing case uses current location and schedule evidence. |
| Export | Generated artifacts retain source versions, owners, and disclosure scopes. |
| External action | Browser uploads use exact reviewed action requests. |
| Receipt | Portal records bind receipt, filename, bytes, and SHA-256. |
| Recovery | A later source-record check resolved one unknown browser result without another submission. |

Obscura does not support local file selection.
The existing provider route switches uploads to Kernel.

Noema does not claim integration portability from these tests.
The human waived second-provider portability for the current roadmap.

## Validation

The focused feature and regression tests passed during implementation.

- Exact decimal calculation, invalid input, division, and date cases passed.
- Private artifact intake and path safety cases passed.
- Raster OCR and email parsing cases passed.
- Artifact source preservation and accessible HTML checks passed.
- Local artifact citation cases passed.
- Browser artifact upload and provider routing cases passed.
- Unknown browser outcome handling passed.
- A failed provider switch now returns `retry_later`.
- Required Task finish tools remain first after the new calculation and artifact tools were added.

| Final check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed. |
| `cargo check-workspace` | Passed after the final tool-order correction. |
| `git diff --check` | Passed. |
| Rust size budget | Passed at `+1,299` production lines, `+343` test lines, and 16 tests. |
| Focused tool-order regression | Passed after correction. |
| `cargo gate-lint` | Reached 24 existing `noema-store` library errors and one existing test error. No Milestone 3 file failed. |
| `cargo gate-test` | Reached two existing missing constants in `noema-api` test code. No test binary ran. |

The direct `noema-runtime` unit run passed every Milestone 3 test.
It also exposed the tool-order regression, which was fixed and retested.

The 2026-08-30 replacement-path validation also passed:

- Three focused `noema-runtime` Luau tests.
- The focused all-features provider schema test.
- `cargo check-workspace`.
- `git diff --check`.
- The live Planner, Executor, and Reviewer path.
- The deliverable-specific accessible HTML checks.

Four unrelated runtime tests remain red in the current worktree.
They cover terminal token timing, a missing provider account fixture, Project event identifiers, and foreground prompt history.

The temporary portal service is stopped.
Its seven receipt records remain under `/var/lib/noema-dev/test-fixtures/milestone-3-portal/`.
The public test route was removed from Caddy.

## Historical exit result

- No accepted generated number depends only on model arithmetic.
- Every uploaded or exported document retains its source, version, owner, and disclosure scope.
- Every required Milestone 3 main path reached reviewer-approved completion.
- The human waived affected-user review only for the controlled accessibility fixture.

## Observed follow-up

An action decline does not carry a human reason into the resumed Task.
The travel correction therefore used Task cancellation and reopen to deliver one natural feedback message.

Add an optional decline note only if another current path needs direct correction without Task reopen.
