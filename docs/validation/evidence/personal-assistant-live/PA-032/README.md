# PA-032 — Research brief

Verdict: **Pass after four bounded connector revisions and a saved brief**

Date: 2026-09-08

Backend: live Go Noema development instance through `/tmp/noema-codex/graphql.sock`.

## Fixture

- Service: synthetic research API, fixture `2026-09-08-research-api-v1`.
- Documentation: `https://futures-laura-raised-controlled.trycloudflare.com/docs`.
- Read endpoints: `/v1/profile` and `/v1/sources`.
- Fixture source: [`scripts/acceptance/run-mock-research-api.ts`](../../../../../scripts/acceptance/run-mock-research-api.ts).
- The fixture is read-only. It contains six dated sources about the Northstar heat-pump rebate program: an official final report, an official correction, two summaries, a dissent, and an outdated draft article.
- Conversation: `conversation:a39407c5e9686f34225273862829bf0f`.
- Documentation read: browser action `action:f1a0683795b6a7bead75e113bdaa9c6b`, approved at revision 1.

## Connector setup and repairs

Noema read the service documentation, proposed a read-only API connection, and
paused for review. The first two proposals exceeded the response-size limit.
Natural follow-ups produced bounded projections. Each accepted revision was
used in a fresh read before the case advanced.

| Revision | Accepted definition digest | Reason for revision |
| --- | --- | --- |
| v1 | `4ff8a7c17c724cd47514c2db34bf1ead64322b0f2ac7c6fc7ff179e5e033534a` | Initial profile and source reads. |
| v2 | `96a1a3dd104752c070017cf7e9e8e3dec3c72da7ab74b107e2cc931e86387010` | Preserve source identity, titles, publishers, and bounded key points. |
| v3 | `a5e752fea427db26d7691a59eebed6c37371e8e7de4a6e86b4f506b5e800af7d` | Map `published_at` and preserve complete bounded summaries and key points. |
| v4 | `492ff52126ad7c9625db030dce00599cde1a60d62713bfa836000e0d1d2a0f9c` | Map the documented `corrects` relation to `correction_of_id`. |

The final active connection is `b065d9f5137475706383ee15af21559f`, definition
revision v4, connection revision 5, policy revision 2. It permits only
`get_profile` and `list_sources`; both are read-only GET operations.

## Fresh source read

Turn `turn:ddb6c1e1cb320fcabf6c0cfa3ddb6169` read the profile and all six
sources through the accepted connection. Results are in transcript items
`conversation_item:2267–2271`.

The read preserved every required field:

- all six identifiers and titles;
- publishers, published dates, source types, and status labels;
- complete bounded summaries and every key point;
- `source-002` with `correction_of_id: source-001`;
- source URLs for every record.

The read showed the official correction as current and the earlier final-rules
record as superseded. It also preserved the disagreement in which Gridline
repeated the old cap while the City Ledger reported the corrected cap.

## Brief and artifact

Turn `turn:0228fda37f386c2d2ea3ba397822b608` produced the synthesis. It:

- selected the latest current official correction as the controlling source;
- stated the corrected `$2,500` and `$1,500` income-tier caps;
- retained the unchanged 40% rate, dates, owner-occupied condition, and
  approved-installer invoice requirement;
- separated confirmed rules from secondary interpretations, forecasts, and
  advocacy;
- explained why the July draft is superseded;
- cited each material claim with source URLs and dates;
- stated remaining uncertainty instead of inventing eligibility or processing
  details.

Noema saved the cited markdown brief as:

- Artifact: `artifact:d19ee2f265edfb8cae5d53b18a49ccb6`
- Version: `artifact_version:7551377a9f8234447c155ee8366beb32`
- File: `northstar-heat-pump-rebate-brief-2026-09-08.md`
- Size: 6,184 bytes

The stored file was inspected under the development home. It contains the
same source-linked synthesis returned in the turn.

## Acceptance checks

| Requirement | Result |
| --- | --- |
| Read service documentation before connecting | Pass. Noema fetched `/docs` before proposing the connection. |
| Generate and review a read-only connection | Pass. The operator accepted the exact v4 proposal. |
| Preserve all six sources | Pass. `list_sources` returned six records in the fresh read. |
| Preserve dates and source types | Pass. All six records include `published_at` and `source_type`. |
| Preserve source identity and correction relation | Pass. IDs, titles, and `source-002 → source-001` are present. |
| Use primary evidence and explain the correction | Pass. The brief prioritizes the current official correction and explains the superseded cap table. |
| Separate facts, interpretations, forecasts, dissent, and outdated coverage | Pass. The brief has explicit sections and source labels. |
| Cite material claims | Pass. Claims include source URLs and publication dates. |
| Save a markdown brief | Pass. Local artifact created and inspected. |
| Avoid external writes | Pass. Both connector operations are read-only; no contact or external write occurred. |
