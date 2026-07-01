# MCP Tool Calibration Autofill Design

## Goal

Add an Autofill action to MCP tool configuration that uses a Noema-hosted LLM
call to suggest read, write, export, and owner resolution settings for all
discovered tools on an MCP server.

Autofill is advisory. It may populate frontend draft fields, but it must not
persist calibration records. The user must review the suggestions and click Save
before any backend calibration state changes.

## User Experience

The MCP tool permissions modal adds an Autofill button next to Save in the
footer. Autofill runs across the full discovered tool set for the current MCP
server so the model can classify tools consistently. While the request is
running, Autofill shows a loading icon and both Autofill and Save are disabled.

When an MCP server is initially added and metadata discovery succeeds, the
permissions modal opens as it does today and automatically triggers Autofill
once for that server. Returned suggestions are applied to the modal's local
draft state. The user can inspect each tool, edit any field, and save all tools
with the existing Save action.

After Autofill succeeds, the modal shows a compact status message:
"Autofill suggestions applied. Review before saving." Tools that still need
attention keep their existing attention badge. If Autofill fails, the modal
keeps the existing default drafts and shows a non-blocking error message so the
user can configure tools manually.

## Backend Contract

Add a GraphQL mutation that accepts an MCP server id and returns suggested
calibrations for the server's known tools. The mutation reads only persisted MCP
server and tool metadata:

- tool id
- name
- description
- input schema
- output schema
- annotations
- metadata fingerprint

The mutation does not call the MCP server and does not write tool calibration
records. It returns suggestions keyed by known `mcp_tool_id` values. Unknown tool
ids, invalid enum values, invalid extractor sources, invalid selector kinds, and
blank extractor paths are rejected before a response reaches the frontend.

## Prompting And Validation

The prompt instructs the model to classify conservatively from metadata only:

- `read` means the tool can bring data from the MCP destination into Noema.
- `write` means the tool can mutate state inside the MCP destination.
- `export` means the tool can share information beyond the MCP destination.
- Use `none` only when the axis clearly does not apply.
- Use `mixed` when trust or ownership depends on runtime contents.
- Use `trusted` or `untrusted` only when the metadata makes the trust boundary
  clear without runtime data.
- Suggest owner extractors only when a deterministic field is present in the
  metadata shape.
- If ownership cannot be resolved, leave extractors empty. Noema will keep mixed
  tools blocked until the user confirms or adds owner resolution.
- Return strict JSON only.

The backend parses a strict JSON response into a typed suggestion structure. It
validates all enum fields against the same MCP calibration enums already used by
manual save. The frontend receives only validated suggestions.

## Frontend Draft Application

The permissions modal adds an Autofill mutation alongside the existing
`mcpTools` query and `saveToolCalibration` mutation. Successful suggestions are
merged into `draftOverrides` for every returned tool.

Autofill replaces editable draft values for suggested tools:

- `readClassification`
- `writeClassification`
- `exportClassification`
- `ownerExtractors`
- `disabled`, if the backend suggestion includes it

The merge does not persist anything. Save continues to call the existing manual
calibration save mutation and remains the only path that writes reviewed
calibrations.

## Initial Add Flow

When `createMcpServer`, OAuth continuation, or retry setup returns
`ready_for_calibration`, the settings page opens the permissions modal. The
modal detects that it was opened for a newly added server and triggers Autofill
once after tools load. Reopening an existing server from the list does not
autofill automatically; the user can click Autofill manually.

Automatic Autofill is best-effort. Failure does not close the modal, block
manual configuration, or mark the server as configured.

## Testing

Backend tests cover:

- GraphQL schema exposes the Autofill mutation and suggestion types.
- The mutation returns validated suggestions for known tools.
- Invalid model output is rejected without writing calibrations.
- Unknown tool ids and invalid enum strings fail validation without returning
  partial suggestions.
- The mutation does not create or update `tool_calibrations`.

Frontend validation covers:

- Autofill button loading and disabled states.
- Successful suggestions populate local drafts.
- Save is still required to persist suggestions.
- Automatic Autofill runs once after newly added MCP setup opens calibration.
- Manual reopen does not auto-run Autofill.
