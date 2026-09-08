# PA-033 — Topic monitoring

Verdict: **Pass**

Date: 2026-09-08

Backend: live Go Noema development instance through
`/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic topic-monitoring API, fixture
  `2026-09-08-topic-monitor-api-v1`.
- Documentation: `https://occupation-irrigation-pix-collectibles.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile` and `/v1/updates`.
- Fixture source: [`scripts/acceptance/run-mock-topic-monitor-api.ts`](../../../../../scripts/acceptance/run-mock-topic-monitor-api.ts).
- The fixture has no utility account and no notification or write operation.
  All profile and update records are synthetic.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.

The fixture state moved through `baseline`, `duplicate`, `changed`, and a
second `changed` read. The first baseline had three records. The duplicate
phase added `update-004`, which repeated `update-002` by canonical URL and
content. The changed phase revised the official supply rate from 12 to 14
cents per kWh and added `updated_at`.

## Connection setup

Noema read the service documentation and proposed the smallest read-only API
connection. The initial accepted definition was
`1f3ebaaca3c0216c6608a81df56d068cf1bc3dc9547df0c4c0d1c89eddf57b06`.

The operator then accepted a focused revision,
`0ef6863fb90dcafba9fecfeea52662d4ea0351f9969186aa6e056917b03c4077`, which
kept `get_profile` and expanded `list_updates` to four bounded records while
preserving `updated_at`. The revision was proposed after Noema inspected the
new documentation and paused for review. The documentation review actions
were `action:dad6f8775763f8152c350a04aef02fe6`,
`action:3c430f6fab843a3052b106a1b58a5466`,
`action:b9439fb13c70d194abe7ee08192fe9a7`, and
`action:e7d3ea111846493253d5a6b5786a01d1`.

The final connection was `bce9c9ab4f03b256acb4d1f7766961d6`, at connection
revision 3 and policy revision 2. It exposed only `get_profile` and
`list_updates`. The data policy was `allow_automatically`, and the unsafe
action policy was `always_ask`. Both operations remained available after the
revision.

## Baseline

Turn `turn:188c413b35ea9a37905210f0793fd955` read the profile and all three
baseline updates through the accepted connection. Noema saved the baseline
without issuing an alert:

- Artifact: `artifact:984f4a7447fe21e81874ac7234b0f6f1`
- Version: `artifact_version:1dfa91a35f2587fb974b1f1fa7d06199`
- File: `riverbend-electricity-rate-monitor-baseline-2026-09-08.md`

The result preserved the topic, household, 900 kWh monthly usage, $10 alert
threshold, three source records, dates, status, summaries, and canonical URLs.

## Duplicate coverage

Turn `turn:b12735a98083f730157985f6a3214770` read all four duplicate-phase
records. Noema matched `update-004` to `update-002` by canonical URL and
content, suppressed the repeated coverage, and returned:

> no material change found; `update-004` is duplicate Daily coverage of
> `update-002`, so it was suppressed and no notice was created or sent

The final item was `conversation_item:2374`. An earlier bounded read
(`turn:047e66ac3ea44368058db9809c79ea03`) returned only three records and is
not used as acceptance evidence.

## Material change

After the fixture moved to `changed`, turn
`turn:f19d1f3c8559b364cd7bd5339c2fb282` read all four records. Noema ignored
the duplicate and identified the official 12-to-14-cent change effective
October 1, 2026. It calculated the impact at 900 kWh as $18 per month, above
the $10 threshold, cited the source URL, and stated that no one was contacted.
The final item was `conversation_item:2380`.

## Unchanged repeat

Turn `turn:ec8db13f55140c3e93aacf5b7358b28b` read the same four records again.
Noema compared the current snapshot with the already reported change and
returned:

> no additional notice issued, the current snapshot is unchanged from the
> confirmed $0.14/kWh update already reported

The final item was `conversation_item:2386`. This turn created no new notice,
artifact, or external contact.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read documentation before connecting | Pass. Noema fetched the service documentation before proposing the connection and revision. |
| Generate, review, and accept a read-only connection | Pass. The accepted connection exposes only the two documented GET operations. |
| Save a baseline without an alert | Pass. The three baseline records were saved as a markdown artifact. |
| Return and suppress duplicate coverage | Pass. The four-record read included `update-004`; Noema suppressed it by canonical URL and content. |
| Report one material change | Pass. The official rate moved from $0.12 to $0.14 per kWh; Noema calculated $18 per month and cited the source. |
| Stay quiet on an unchanged repeat | Pass. The follow-up issued no additional notice. |
| Avoid writes and contact | Pass. The fixture is read-only, and every turn explicitly made no external contact. |

No real utility, publisher, household, payment, or notification account was
used.
