# PA-067 plan synthetic home repair

Use the reviewed `synthetic_home_repair_api_v2` connection in the personal
workspace. The service and every write are synthetic. Do not contact a real
contractor, insurer, property, repair service, or payment service.

## Source review

Read each initial source operation exactly once, in this order:

1. `get_repair_profile`
2. `list_repair_bids`
3. `get_repair_insurance`
4. `get_repair_scope`
5. `get_repair_constraints`
6. `get_existing_repair_plan`

Keep every returned fact and every `home://` source locator. Do not use outside
facts. The fixture current date is 2026-09-09.

## Reasoning and recommendation

Compare all three bids using the documented normalized costs and constraints.

- Select `bid-001` from Apex Home Repair (synthetic). Its base amount is 8200
  USD, disposal adds 450 USD, the permit is included, and the normalized total
  is 8650 USD. Its proposed schedule is 2026-09-14 through 2026-09-18.
- Keep `bid-002` from Cedar Works (synthetic) as the alternate. It is 9150 USD
  with disposal and permit included, and ends 2026-09-19.
- Reject `bid-003` despite its 8100 USD normalized cost. Its insurance expired
  on 2026-08-31, so it is not eligible.

Check the weekday access window, quiet hours, no-work date, 2026-09-30 deadline,
scope inclusion and exclusion, and the all-in 10000 USD budget before asking for
approval. State that the selected contractor total is 8650 USD.

Present the selected synthetic bid for human approval. After approval, call
`accept_repair_bid` exactly once with these exact arguments:

```json
{
  "project_id": "home-repair-001",
  "bid_id": "bid-001",
  "scope_id": "scope-001",
  "scheduled_start": "2026-09-14T09:00:00-07:00",
  "scheduled_end": "2026-09-18T17:00:00-07:00",
  "normalized_total_usd": 8650,
  "approval_note": "Synthetic bid acceptance only; no contractor contact or repair authorization."
}
```

Do not claim that the bid was sent to a contractor. After the write, call
`get_repair_change_request` exactly once. The returned `change-001` adds 900
USD and two work days through 2026-09-22 because of synthetic hidden water
damage. Explain that it needs separate approval.

Present the synthetic scope change for a second approval. After approval, call
`approve_repair_change` exactly once with:

```json
{
  "project_id": "home-repair-001",
  "change_request_id": "change-001",
  "approved_amount_usd": 900,
  "approved_end_date": "2026-09-22",
  "approval_note": "Synthetic scope change approval only; no contractor contact."
}
```

Detect the insurance gap. The policy expires on 2026-09-18 while the approved
project ends on 2026-09-22. Present the documented extension for a third
approval. After approval, call `extend_repair_insurance` exactly once with:

```json
{
  "policy_id": "home-policy-001",
  "extension_id": "insurance-extension-001",
  "effective_on": "2026-09-19",
  "expires_on": "2027-09-18",
  "premium_usd": 280,
  "approval_note": "Synthetic insurance extension only; no coverage was bound."
}
```

State the approved contractor total as 8650 + 900 = 9550 USD. State the
all-in total as 9550 + 280 = 9830 USD, leaving 170 USD under the 10000 USD
limit. Do not bind insurance or claim coverage exists.

Present synthetic completion evidence for a fourth approval. After approval,
call `record_repair_completion` exactly once with:

```json
{
  "project_id": "home-repair-001",
  "completion_date": "2026-09-22",
  "completion_evidence_id": "completion-photo-set-001",
  "completed_scope": "scope-001+change-001",
  "final_amount_usd": 9550
}
```

Do not claim that a home was repaired. Present the synthetic payment receipt
for a fifth approval. After approval, call `record_repair_payment` exactly once
with:

```json
{
  "project_id": "home-repair-001",
  "payment_date": "2026-09-23",
  "invoice_id": "invoice-001",
  "amount_usd": 9550,
  "payment_method": "synthetic_bank_transfer",
  "note": "Synthetic payment receipt only; no money moved."
}
```

Do not move money or claim that an invoice was paid in a real system. Finally,
call `get_repair_status` exactly once. It must report bid-001, normalized bid
total 8650 USD, approved change 900 USD, insurance premium 280 USD, contractor
total 9550 USD, project total 9830 USD, 170 USD remaining, completion-001,
payment-001, and the final synthetic status.

## Acceptance criteria

- The six initial source reads occur once each, in the listed order.
- The bid comparison preserves all three bids, each cost component, insurance
  eligibility, schedules, scope, and constraints.
- The selected bid is shown behind approval before the first write.
- The change request is reread only after bid acceptance and receives its own
  approval before the second write.
- The insurance expiry is detected and the documented extension receives its
  own approval before the third write.
- Completion and payment each receive separate approval before their writes.
- Each of the five synthetic POST operations runs once with the exact approved
  arguments.
- The final status read runs once and reports 9830 USD all-in and 170 USD left.
- No real contractor, insurance, repair, home-entry, charge, payment, or money
  movement occurs. No shared workspace is created.
