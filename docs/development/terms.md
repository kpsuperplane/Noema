# Plain Code Terms

This document defines the Noema terms for new prose and changed identifiers.
Use one approved term for one meaning.

Do not make a global text replacement. Keep an external protocol term when
Noema implements that protocol.

## Words to avoid

Do not use these words in new prose or changed identifiers:

- `idempotency`
- `projection`

Use a direct term that describes the specific behavior or data.

## Approved terms

### Tasks

Tasks are the product for queued, scheduled, delegated, or background activity.

Use `Task` in a changed identifier. Do not add `Work` when the word means the
Tasks product.

### Stored state

Stored state is the source data in SQLite.

Do not use `canonical state` for this meaning. Use `canonical` only when a
standard technical meaning requires it.

### Current-run check

A current-run check proves that a state change belongs to the current task run.
It includes the necessary task version, run, worker claim, and requirements.

Use `TaskRunCheck` in a changed identifier. Do not add `task fence` or
`WorkRunFence` for this meaning.

### Action request

An action request is one saved tool call that Noema must check. It stores the
exact input, review result, approval, execution state, and final output.

Use `ActionRequest` in a changed identifier. Do not add `governed action` or
`GovernedAction` for this meaning.

### Correction review

A correction review is the exact review that requested a correction run.

Use `correction_review_id` in a changed identifier. Do not add `review lineage`
or `triggering_review_id` for this meaning.

### Provider schema conversion

Provider schema conversion makes a source-schema copy for one model service.

Use `convert_provider_schema` in a changed identifier. Do not add `schema
lowering` for this meaning.

### Full provider conversion

A full provider conversion does not remove or weaken a source rule.

Use `convert_schema_fully` in a changed identifier. Do not add `strict lowering`
or `convert_schema_exactly` for this meaning.

### Final source-schema check

A final source-schema check validates source-form tool input before invocation.

Use `validate_tool_input` in a changed identifier. Do not add `canonical runtime
validation` for this meaning.

## Context-dependent words

Use the term that states the direct meaning.

| Existing phrase | Approved phrase |
| --- | --- |
| Canonical database state | Stored state or source data |
| Canonical tool schema | Source schema |
| Canonical arguments | Saved exact arguments |
| Canonical file | Source file |
| Canonical JSON encoding | Normalized JSON encoding |
| Strict provider schema | Exact provider schema |
| Strict JSON parser | Keep this term for a named strict parser mode |
| Workflow | Keep this standard technical term |
| Ordinary English work | Keep this word when it does not name the Tasks product |

Standard technical terms can remain when they give a necessary distinction.
Examples include `payload`, `principal`, and `adapter`.
