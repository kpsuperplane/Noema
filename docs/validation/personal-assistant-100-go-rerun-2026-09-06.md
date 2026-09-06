# Personal assistant 100-task live Go rerun

Date: 2026-09-06

This report records a live screen of all 100 rows in the [personal-assistant task ledger](../difficult-digital-personal-assistant-tasks.md).

This is not the 241-row Go server verification suite.

## Result

The current Go server completed ten read-only Chat batches that covered every ledger row.

| Result | Count | Meaning |
| --- | ---: | --- |
| PASS | 4 | The current home contained enough non-empty evidence for the requested outcome. |
| PARTIAL | 13 | Some current evidence existed, but a required source or part of the outcome was missing. |
| BLOCKED | 83 | The current home had no case data for the requested task. |
| **Total** | **100** | Every row was screened. |

The four current live passes are Tasks **15, 19, 33, and 35**.

The original report still records **80 historical Verified tasks**. This run does not replace that score. It shows that the current live home does not contain the old fixtures needed to replay most of those cases.

A blocked row is therefore an environment and fixture result. It is not evidence of a Go regression.

## Live backend

- Go binary: `/run/noema-dev/noema`
- Running process: PID `3499811`
- Source checkout: commit `eeb9cc52a3eeb2538bb57faa50272c0191702911`
- Protected GraphQL socket: `/var/lib/noema-dev/run/graphql.sock`
- Socket mode: `0600`, owned by `noema-dev:noema-dev`
- Primary conversation: `conversation:9a8adece587d96886957b3a308c694b1`
- Health query: `query { __typename }` returned `QueryRoot`

The authenticated HTTP path also completed a read-only smoke turn. A separate live Task completed Planner, Executor, and Reviewer stages as `task:5cd83b966427221053fb0e99cf8fa005`.

## What changed to make the run work

The live server was already healthy. The case runner had two live-environment problems:

1. Its GraphQL selection used the same `state` field across incompatible intervention types. The Go schema rejected that selection.
2. The protected socket has no browser authentication context. Its human-intervention query therefore failed even when the Task query was healthy.

`scripts/run-live-noema-case.ts` now uses non-conflicting aliases and a socket-safe governed-action query. It also supports an authenticated HTTP cookie for environments where the socket is unavailable.

No production server code changed for this rerun.

## Test method

Each batch used the normal live `sendConversationTurn` path. The prompt named each task family and required one result per number.

Every prompt required:

- current synthetic Noema records only;
- no Task delegation;
- no messages, uploads, payments, bookings, cancellations, submissions, deletions, or settings changes;
- no external service calls;
- `PASS` only when the main outcome had complete, current, non-empty evidence;
- `PARTIAL` or `BLOCKED` when evidence was incomplete or absent.

The batch turn IDs are:

| Rows | Turn |
| --- | --- |
| 1–10 | `turn:c117d84d00583d3e03deabd31e7aa79e` |
| 11–20 | `turn:fbcf558bf069711f95c2579c5ab20db9` |
| 21–30 | `turn:ca6a48233997a243c8965355952944e3` |
| 31–40 | `turn:48e1a6046b3c8bd35d8d774596ac8314` |
| 41–50 | `turn:9ec221c7b1c9599063ba1675c74d993f` |
| 51–60 | `turn:c9a1b278decd547c090e60269c9e0e60` |
| 61–70 | `turn:beae4619eaafd25cea3bc8bdb05c6cf8` |
| 71–80 | `turn:009cfb91fe6d02b53c56ecdd3951a1e9` |
| 81–90 | `turn:9520003210d98fe1b90e38c67da12490` |
| 91–100 | `turn:895bf94522394cdfb0173741feb89d45` |

These are capability screens, not full acceptance promotions. The old acceptance contract requires populated source fixtures, source receipts, and write or reconciliation variants where applicable.

## Per-task results

### Tasks 1–10

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 1 | Build a daily operational brief | PARTIAL | Two current Task deadlines exist. Calendar and availability evidence is missing. |
| 2 | Maintain a promise register | BLOCKED | No current promise, conversation, or source record exists. |
| 3 | Capture actions, decisions, and dates | PARTIAL | Two current actions and dates exist. No decision record exists. |
| 4 | Maintain a reply and action queue | PARTIAL | Task deadlines support a small queue. Reply sources and priority fields are missing. |
| 5 | Audit calendar conflicts and hidden load | BLOCKED | No calendar, location, travel-time, or availability record exists. |
| 6 | Create a realistic daily plan | PARTIAL | Two Task estimates and deadlines exist. Calendar blocks and working time are missing. |
| 7 | Replan after disruption | BLOCKED | No changed commitment or disruption trigger exists. |
| 8 | Produce a weekly preview and review | BLOCKED | No current weekly calendar, completed-work, or review input exists. |
| 9 | Track deadlines and recurring obligations | PARTIAL | Two deadlines exist. No renewal or recurring-obligation record exists. |
| 10 | Review personal goals and adjust the plan | PARTIAL | A stargazing interest exists in Memory. Goals, progress, and a review period are missing. |

### Tasks 11–20

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 11 | Resolve conflicting requests | BLOCKED | Current records show conflicting launch estimates, not competing requests with decision authority. |
| 12 | Monitor an important conversation | BLOCKED | No thread, participant, monitoring rule, or follow-up source exists. |
| 13 | Coordinate a multi-person meeting | BLOCKED | No attendee list, availability, location, or time-zone source exists. |
| 14 | Prepare a meeting brief and agenda | BLOCKED | No meeting purpose, participants, prior context, or agenda input exists. |
| 15 | Prepare audience-specific updates | PASS | Current release records support a bounded customer update. The update can retain the 240-sample fact, conflicting unapproved dates, and missing sign-off and rollout evidence. |
| 16 | Record decisions and close follow-ups | BLOCKED | No meeting notes, decisions, owners, or follow-up commitments exist. |
| 17 | Maintain a relationship brief | BLOCKED | No relationship profile, interaction history, or person-linked context exists. |
| 18 | Maintain a relationship follow-up plan | BLOCKED | No cadence, commitment, or next-contact source exists. |
| 19 | Produce an evidence-based project status | PASS | Two linked release Tasks provide current progress, the 240-sample fact, conflicting unapproved dates, and missing sign-off and rollout evidence. |
| 20 | Detect project risks and dependencies | PARTIAL | Current records show schedule and readiness risks. No dependency map, owner, or mitigation date exists. |

### Tasks 21–30

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 21 | Maintain a decision log | BLOCKED | No decision entries, replacement links, dates, or decision authority exist. |
| 22 | Turn an ambiguous goal into a project plan | BLOCKED | No current goal, scope, milestones, owners, or plan exists. |
| 23 | Assemble a deliverable from scattered material | BLOCKED | No source artifacts or deliverable specification exists. |
| 24 | Coordinate a multi-reviewer approval | BLOCKED | No approver roster, review criteria, stages, or submission artifact exists. |
| 25 | Build an employer expense packet | BLOCKED | No expense receipt, policy, reimbursement destination, or submission record exists. |
| 26 | Prepare an onboarding, offboarding, or handoff package | BLOCKED | No role, asset, access, recipient, or checklist source exists. |
| 27 | Maintain credentials and compliance obligations | BLOCKED | No credential inventory, expiry date, rule, or renewal source exists. |
| 28 | Run a job-search pipeline | BLOCKED | No job target, application, employer contact, interview, or pipeline record exists. |
| 29 | Prepare tailored application packets | BLOCKED | No job posting, experience source, or application destination exists. |
| 30 | Compare job offers and prepare negotiation | BLOCKED | No offer terms, compensation, benefits, preferences, or constraints exist. |

### Tasks 31–40

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 31 | Compare a major product or service | BLOCKED | No current options, personal constraints, budget, or comparison criteria exist. |
| 32 | Produce a current research brief | BLOCKED | No current research request or source corpus exists. Historical research does not count. |
| 33 | Monitor a topic for material changes | PASS | Current release records show a real date change and continued missing rollout evidence. |
| 34 | Build a literature review and evidence map | BLOCKED | No current research question, literature corpus, or evidence-map source set exists. |
| 35 | Fact-check a claim set | PASS | Both current release records corroborate 240 staging samples and show that both launch dates remain unapproved. |
| 36 | Extract a structured inventory | BLOCKED | No current multi-document or site corpus exists. |
| 37 | Analyze a personal dataset | BLOCKED | No current dataset, analysis question, or data-access source exists. |
| 38 | Maintain a reading and newsletter digest | BLOCKED | No reading list, subscriptions, feeds, or digest preferences exist. |
| 39 | Maintain an adaptive learning plan | PARTIAL | A stargazing interest exists. No objective, baseline, resources, cadence, or progress signal exists. |
| 40 | Compare courses or credentials | BLOCKED | No program options, criteria, costs, deadlines, or learner constraints exist. |

### Tasks 41–50

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 41 | Maintain household cash flow and bills | PARTIAL | One invoice calculation exists. Balances, bill schedule, due dates, and cash-flow history are missing. |
| 42 | Build an annual tax-readiness packet | PARTIAL | One invoice source exists. Tax, income, deduction, filing, and document records are missing. |
| 43 | Audit subscriptions and cancellations | BLOCKED | No subscription inventory, terms, payment records, or cancellation destination exists. |
| 44 | Review insurance coverage | BLOCKED | No policy, limit, beneficiary, premium, or risk record exists. |
| 45 | Reconcile an insurance claim | BLOCKED | No claim, policy, loss, correspondence, or status source exists. |
| 46 | Find and maintain benefits | BLOCKED | No eligibility profile, program rule, application, or renewal record exists. |
| 47 | Consolidate retirement records | BLOCKED | No account statement, inventory, rollover term, or decision criterion exists. |
| 48 | Maintain credit and debt records | BLOCKED | No credit report, debt balance, payment schedule, dispute, or creditor record exists. |
| 49 | Maintain official documents and licenses | BLOCKED | No document inventory, expiry date, renewal rule, or issuing authority exists. |
| 50 | Escalate a consumer dispute | BLOCKED | No purchase, merchant dispute, payment record, or escalation channel exists. |

### Tasks 51–60

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 51 | Maintain an important-affairs and estate map | BLOCKED | No affairs inventory, estate document, account list, fiduciary contact, or location map exists. |
| 52 | Administer a deceased person's accounts | BLOCKED | No identity, authority document, account, benefit, notice, or estate record exists. |
| 53 | Consolidate a medical record | BLOCKED | No medical history, diagnosis, clinician, laboratory, imaging, or release source exists. |
| 54 | Maintain a medication and refill plan | BLOCKED | No medication, prescription, prescriber, pharmacy, refill, or adherence record exists. |
| 55 | Prepare a medical appointment brief | BLOCKED | No appointment purpose, symptom, history, question, clinician, or test result exists. |
| 56 | Coordinate referrals and tests | BLOCKED | No referral, order, specialist, test schedule, transport, or contact exists. |
| 57 | Monitor a care plan | BLOCKED | No care plan, milestone, monitoring data, clinician, or follow-up exists. |
| 58 | Compare care providers | BLOCKED | No care need, provider option, network, location, availability, or comparison rule exists. |
| 59 | Compare health plans | BLOCKED | No plan option, premium, network, benefit, medication, or eligibility record exists. |
| 60 | Build a health authorization or appeal packet | BLOCKED | No authorization, claim, denial, clinical evidence, policy term, deadline, or appeal record exists. |

### Tasks 61–70

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 61 | Coordinate a safe care transition | BLOCKED | No discharge plan, clinical instruction, medication, appointment, or authorized contact exists. |
| 62 | Maintain a caregiver schedule | BLOCKED | No caregiver roster, care need, schedule, responsibility, availability, or escalation plan exists. |
| 63 | Summarize care-recipient changes | BLOCKED | No change record, recipient list, or communication consent exists. |
| 64 | Prepare a treatment decision brief | BLOCKED | No diagnosis, treatment option, clinical evidence, preference, or second-opinion source exists. |
| 65 | Maintain an asset and recall inventory | BLOCKED | No asset list, receipt, warranty, purchase, recall, or location record exists. |
| 66 | Run preventive home maintenance | BLOCKED | No home inventory, maintenance history, seasonal task, service, or property constraint exists. |
| 67 | Coordinate a home repair project | BLOCKED | No repair scope, property detail, budget, contractor, permit, timeline, or approval exists. |
| 68 | Optimize utilities and communication services | BLOCKED | No account, usage, price, contract, availability, or household requirement exists. |
| 69 | Maintain a meal and grocery plan | BLOCKED | No dietary need, pantry inventory, preference, budget, schedule, or retailer source exists. |
| 70 | Coordinate the vehicle lifecycle | BLOCKED | No vehicle identity, registration, insurance, service, mileage, ownership, or replacement rule exists. |

### Tasks 71–80

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 71 | Coordinate recurring pet care | BLOCKED | No pet, care need, veterinarian, caregiver, feeding, medication, or recurrence record exists. |
| 72 | Complete a return or warranty request | BLOCKED | No purchase, merchant, product, defect, warranty, receipt, or destination exists. |
| 73 | Manage recurring household services | BLOCKED | No service inventory, provider, contract, cadence, price, or requirement exists. |
| 74 | Maintain emergency readiness | BLOCKED | No household, location, hazard, supply, contact, evacuation, or preference record exists. |
| 75 | Build a live itinerary | PARTIAL | A current French travel preference exists. No confirmation, destination, date, or transport record exists. |
| 76 | Plan and book a multi-leg trip | PARTIAL | A current French travel preference exists. No destination, dates, travelers, budget, route, or booking criteria exists. |
| 77 | Monitor disruptions and prepare rebooking | BLOCKED | No trip confirmation, disruption notice, traveler constraint, or rebooking term exists. |
| 78 | Maintain international travel readiness | BLOCKED | No nationality, passport, visa, destination, vaccine, entry rule, or expiry record exists. |
| 79 | Track travel credits and refunds | BLOCKED | No booking, credit, refund, claim, payment, deadline, or provider record exists. |
| 80 | Coordinate group or accessible travel | BLOCKED | No group roster, accessibility need, destination, date, budget, transport, or lodging record exists. |

### Tasks 81–90

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 81 | Plan a personal event | BLOCKED | No event purpose, date, guest, venue, budget, preference, or vendor source exists. |
| 82 | Run a move and propagate changes | BLOCKED | No move date, origin, destination, lease, inventory, account, or recipient source exists. |
| 83 | Maintain a housing search | BLOCKED | No location, budget, household requirement, listing, financing, lease, or search rule exists. |
| 84 | Complete a trip or move closeout | BLOCKED | No completed trip or move, booking, receipt, account change, return, or checklist exists. |
| 85 | Maintain a family schedule | BLOCKED | No family roster, schedule, transport need, location, vehicle, or caregiver constraint exists. |
| 86 | Turn school communications into a digest | BLOCKED | No school message, student identity, calendar, notice, deadline, or preference exists. |
| 87 | Coordinate childcare and activities | BLOCKED | No child, care need, activity, schedule, enrollment, transport, or caregiver record exists. |
| 88 | Balance household and care responsibilities | BLOCKED | No household roster, care responsibility, schedule, workload, availability, or preference exists. |
| 89 | Run an education or scholarship campaign | BLOCKED | No student profile, target, scholarship, eligibility, deadline, or campaign material exists. |
| 90 | Maintain family records and permission packets | BLOCKED | No family record, dependent, consent, permission, expiry, or recipient source exists. |

### Tasks 91–100

| # | Task | Result | Current evidence or gap |
| ---: | --- | --- | --- |
| 91 | Coordinate community or volunteer participation | BLOCKED | No organization, role, availability, event, contact, or participation preference exists. |
| 92 | Maintain a relationship and occasion plan | BLOCKED | No relationship profile, date, occasion preference, contact history, or commitment exists. |
| 93 | Organize and preserve files, email, and photos | BLOCKED | No file, email, photo, storage, retention, backup, or organization inventory exists. |
| 94 | Maintain an account and security inventory | BLOCKED | No account, credential, recovery method, security status, or ownership source exists. |
| 95 | Coordinate identity-theft recovery | BLOCKED | No compromised account, incident, affected service, evidence, or recovery case exists. |
| 96 | Review privacy settings and data exposure | BLOCKED | No privacy setting, permission, sharing record, exposure finding, or review scope exists. |
| 97 | Complete device replacement and migration | BLOCKED | No device inventory, replacement, backup, migration scope, or transfer requirement exists. |
| 98 | Maintain a personal knowledge base | PARTIAL | Memory has durable stargazing and French travel-note entries. A broader capture, structure, and coverage workflow is absent. |
| 99 | Prepare a digital-legacy plan | BLOCKED | No digital asset, access map, legal authority, beneficiary, preservation, or executor record exists. |
| 100 | Make digital information accessible | BLOCKED | No accessibility need, audience, content inventory, format requirement, or destination exists. |

## Interpretation

The live run confirms three things:

1. The Go backend can accept, execute, and complete the 100-row screening path.
2. Current Project and Task records support the same evidence-bound reasoning used by the old project cases.
3. The current Go home is not a copy of the old acceptance home. Most historical cases cannot be replayed until their provider-neutral fixtures are restored.

The next credible parity step is to create one canonical, provider-neutral fixture and expected result for each ledger row. Until then, the historical 80/100 score remains the correct record of prior acceptance, while this report is the current live-Go fixture-availability result.
