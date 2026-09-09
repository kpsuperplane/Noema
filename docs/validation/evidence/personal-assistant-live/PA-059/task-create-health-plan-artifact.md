# PA-059 save the synthetic health-plan comparison

Create exactly one local Markdown artifact with `artifact.create_local_file`.
Use only the verified facts below. Do not call the health-plan connector,
enroll, cancel coverage, submit a claim, contact an insurer, pay a fee, give
legal or medical advice, or choose a plan for the person.

The artifact must preserve the profile, both complete plan records, all seven
usage records, every `plan://` source locator, the calculation method and
component arithmetic, both expected totals, both worst-case totals, the
excluded-drug cash cost, the network limits, the caveats, and the
person-controlled decision boundary. Separate returned facts from calculated
values and questions to verify.

Verified source facts:

- Fixture: `2026-09-09-health-plan-comparison-api-v1`.
- Fixture URL:
  `https://policies-porter-defining-guided.trycloudflare.com`.
- Case: `health-plan-compare-001`; patient label `Jordan Lee (synthetic)`;
  current date `2026-09-09`; scenario period `2027 calendar year`; household
  size `1`.
- Goal: `Compare two synthetic health plans for the supplied care and
  medication scenario.`
- Network preference: `Use in-network care for the expected estimate.`
- Expected-cost rule: twelve months of premium plus covered service and
  prescription cost shares; apply medical and prescription deductibles before
  coinsurance when a claim type uses them; add the supplied cash estimate for
  an excluded drug outside plan coverage.
- Worst-case rule: twelve months of premium plus the plan out-of-pocket
  maximum for covered in-network care; add excluded-drug cash cost because it
  is outside the plan maximum.
- Decision boundary: `Do not enroll, cancel coverage, submit a claim, contact
  an insurer, give legal or medical advice, or choose a plan for the person.`
- Profile source: `plan://preferences/health-plan-compare-001`.
- Plan `plan-001`, Harbor Standard PPO (synthetic): monthly premium `$360`;
  medical deductible `$1,200`; prescription deductible `$250`; out-of-pocket
  maximum `$5,000`; network scope `in_network_only`; out-of-network cost share
  `not_covered`; primary-care copay `$25`; specialist copay `$60`; urgent-care
  copay `$75`; lab coinsurance `20%`; generic copay `$10`; preferred-brand
  copay `$35`; specialty copay `$250`; excluded drug `drug-003`,
  `Examplebiologic (synthetic)`; excluded-drug cash price `$1,800` per unit;
  network note `The plan covers the expected estimate only for in-network care.
  Out-of-network care is not covered and may create full charges.`; source
  `plan://plans/plan-001`.
- Plan `plan-002`, Harbor Choice EPO (synthetic): monthly premium `$220`;
  medical deductible `$2,500`; prescription deductible `$500`; out-of-pocket
  maximum `$7,500`; network scope
  `in_network_with_no_out_of_network_benefit`; out-of-network cost share
  `not_covered`; primary-care copay `$40`; specialist copay `$45`; urgent-care
  copay `$75`; lab coinsurance `30%`; generic copay `$5`; preferred-brand
  copay `$25`; specialty copay `$250`; excluded drug `none`; excluded-drug
  cash price `$0`; network note `The plan has no out-of-network benefit. The
  expected estimate assumes every service uses an in-network provider.`; source
  `plan://plans/plan-002`.
- Usage `visit-001`: primary-care visit, `4` units, allowed unit cost `$150`,
  rule `primary_care_copay`, source `plan://scenario/visit-001`.
- Usage `visit-002`: specialist visit, `2` units, allowed unit cost `$220`,
  rule `specialist_copay`, source `plan://scenario/visit-002`.
- Usage `visit-003`: urgent-care visit, `1` unit, allowed unit cost `$300`,
  rule `urgent_care_copay`, source `plan://scenario/visit-003`.
- Usage `lab-001`: laboratory panel, `2` units, allowed unit cost `$100`, rule
  `medical_deductible_then_coinsurance`, source `plan://scenario/lab-001`.
- Usage `drug-001`: Examplestatin (synthetic), drug `drug-001`, tier `generic`,
  `12` units, allowed unit cost `$120`, rule `prescription_generic`, source
  `plan://scenario/drug-001`.
- Usage `drug-002`: Exampleinhaler (synthetic), drug `drug-002`, tier
  `preferred_brand`, `12` units, allowed unit cost `$210`, rule
  `prescription_preferred_brand`, source `plan://scenario/drug-002`.
- Usage `drug-003`: Examplebiologic (synthetic), drug `drug-003`, tier
  `specialty`, `4` units, allowed unit cost `$1,800`, rule
  `prescription_specialty`, source `plan://scenario/drug-003`.

Calculated values to preserve:

- The two lab rows total `$200`, below both medical deductibles, so the
  scenario applies no lab coinsurance.
- Harbor Standard PPO: annual premium `$4,320`; covered member scenario cost
  `$1,035`; covered scenario cost `$5,355`; excluded-drug cash estimate
  `$7,200` (`4 × $1,800`); expected annual member cost **$12,555**; worst-case
  annual exposure **$16,520** (`$4,320 + $5,000 + $7,200`).
- Harbor Choice EPO: annual premium `$2,640`; covered member scenario cost
  `$1,885`; covered scenario cost and expected annual member cost **$4,525**;
  excluded-drug cash cost `$0`; worst-case annual exposure **$10,140**
  (`$2,640 + $7,500`).
- Both plans have no out-of-network benefit in the returned records. The
  expected estimate assumes in-network care.
- Harbor Standard PPO excludes `drug-003`; Harbor Choice EPO covers it at a
  `$250` specialty copay per unit.
- Expected cost is separate from worst-case exposure. The PPO excluded-drug
  cash estimate is outside insurance cost sharing and outside the plan maximum.

State that these are synthetic calculations, not an eligibility result, plan
recommendation, clinical judgment, legal advice, or enrollment decision. Include
questions about network participation, the drug exclusion and any exceptions,
copay and deductible application, quantity limits, out-of-pocket maximum scope,
and final 2027 terms. State that the person makes the final plan choice.

Use a descriptive artifact title. Return the artifact ID, version ID, byte
count, and content digest. If no digest is returned, state that it was
unavailable.

## Success conditions

- Exactly one local Markdown artifact is created.
- The artifact contains every verified field, all seven usage records, all
  source locators, calculation components, expected totals, worst-case totals,
  excluded-drug cash cost, network limits, caveats, and questions.
- No connector call or external state change occurs during creation.
