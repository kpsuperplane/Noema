#!/usr/bin/env bun

/**
 * A provider-neutral synthetic API for the remaining personal-assistant cases.
 * It records bounded context reads and approval-gated case actions. It never
 * contacts a real provider or performs a real-world action.
 */

const port = Number(process.env.NOEMA_CASE_WORKFLOW_MOCK_PORT ?? "3796");
const fixture = "2026-09-10-case-workflow-api-v1";

type CaseContext = {
  case_id: string;
  case_title: string;
  facts: string;
  constraints: string;
  expected_outcome: string;
};

type CaseState = {
  actions: Array<{ actionId: string; body: Record<string, unknown> }>;
};

const contexts: Record<string, CaseContext> = {
  "PA-074": { case_id: "PA-074", case_title: "Emergency readiness", facts: "Two evacuation routes; mobility need; dog; expired supplies; a later road closure affects route A.", constraints: "Use an accessible route, include the dog plan, replace expired supplies, and save an offline-readable contact packet.", expected_outcome: "Select route B after the closure, replace expired supplies, preserve accessibility, and verify the saved packet." },
  "PA-075": { case_id: "PA-075", case_title: "Itinerary", facts: "Flight, hotel, and rail confirmations; one duplicate booking email; a timezone change; airport transfer details are missing.", constraints: "Merge only the duplicate, preserve booking references and local times, and obtain transfer details before finalizing the timeline.", expected_outcome: "Produce one sourced itinerary with feasible local-time ordering and a documented transfer gap." },
  "PA-076": { case_id: "PA-076", case_title: "Trip booking", facts: "Two travel legs and lodging; total budget 1200 USD; nonstop preference; accessibility limit; first mock booking returns a 502 after commit.", constraints: "Choose a feasible total, approve one exact mock booking, reconcile the uncertain 502 by reading status, and never resubmit.", expected_outcome: "One synthetic booking and receipt with no duplicate submission." },
  "PA-077": { case_id: "PA-077", case_title: "Travel disruption", facts: "A flight is canceled; a downstream hotel remains; three alternatives exist and one violates the access need; an unchanged alert follows.", constraints: "Compare feasible alternatives and full costs, change the mock booking once, reconcile the hotel, and suppress an unchanged alert.", expected_outcome: "One accessible replacement and reconciled downstream reservation." },
  "PA-078": { case_id: "PA-078", case_title: "Entry readiness", facts: "Synthetic nationality, transit point, passport expiry, pinned visa-rule version, and an updated entry notice.", constraints: "Apply only the pinned mock rules, flag the validity gap, and update the checklist after the rule change.", expected_outcome: "A sourced checklist that states uncertainty and does not claim legal clearance." },
  "PA-079": { case_id: "PA-079", case_title: "Travel value recovery", facts: "A 150 USD restricted credit expires in 30 days; an 80 USD refund is pending; an insurance claim covers the same loss.", constraints: "Separate credit from cash, prevent a double claim, submit only an eligible mock request, and remind before expiry.", expected_outcome: "One eligible synthetic request plus separate refund and credit tracking." },
  "PA-080": { case_id: "PA-080", case_title: "Accessible group travel", facts: "Three travelers can travel on Nov 6, Nov 13, or Nov 20, but all three are available together only Nov 13; wheelchair access is confirmed for the selected hotel; transport plus hotel totals 1080 USD; fixed budget is 1200 USD; an unconfirmed hotel option costs 900 USD.", constraints: "Choose Nov 13, use only the confirmed wheelchair-accessible hotel, stay within 1200 USD, and reserve only that feasible plan.", expected_outcome: "A feasible sourced plan for Nov 13 at 1080 USD with confirmed accessibility." },
  "PA-081": { case_id: "PA-081", case_title: "Personal event", facts: "Twenty guests initially require a venue; venue A holds 18 for 300 USD and is undersized; venue B holds 24 for 400 USD; the approved caterer costs 420 USD and supports vegetarian and nut-free meals; two guests later cancel, leaving 18; the total budget is 1000 USD; the deposit is refundable only for venue B.", constraints: "Reject venue A, use the capacity-safe approved vendor plan, revise the final count to 18, and reconcile the 1000 USD budget and refund terms.", expected_outcome: "A capacity-safe event plan using venue B with a reconciled synthetic closeout." },
  "PA-082": { case_id: "PA-082", case_title: "Move coordination", facts: "Twelve address changes cover utilities, school, insurance, subscriptions, and records; the old address remains active through Aug 31; electricity and water at the new address start Aug 20; identity A owns utilities and identity B owns school and insurance; each target has one supplied provider reference.", constraints: "Start new utilities before ending old utilities, sequence the other changes after the move date, submit each scoped change to its supplied identity, and never duplicate a target.", expected_outcome: "Twelve acknowledged changes with preserved service overlap and no identity mix-up." },
  "PA-083": { case_id: "PA-083", case_title: "Housing search", facts: "Listings A-F have total monthly costs of 1750, 1925, 1690, 1810, 1785, and 1650 USD; only A, C, and F allow pets; commute times are 35, 25, 42, 38, 45, and 39 minutes; F is stale and unavailable; C has a 100 USD pet fee and verified availability; the ceiling is 1800 USD and commute limit is 40 minutes.", constraints: "Apply the rent, fee, pet, commute, and availability constraints, exclude stale listing F, shortlist valid options, and submit one application only for a verified choice.", expected_outcome: "A sourced shortlist and one tracked application for a valid available listing." },
  "PA-084": { case_id: "PA-084", case_title: "Trip or move closeout", facts: "Receipts total 900 USD, including one duplicated 100 USD receipt; a 300 USD deposit is still open; a 50 USD refund is outstanding; the utility cancellation has a provider reference but no completion receipt; all other services have closeout receipts.", constraints: "Count unique spend as 800 USD, keep the deposit and refund as separate open money items, verify the utility cancellation, and close only resolved items.", expected_outcome: "A closeout ledger with unique spend and unresolved money/service items kept open." },
  "PA-085": { case_id: "PA-085", case_title: "Family transport", facts: "Child A leaves school at 15:00 and Child B at 15:15; one vehicle is available; custody allows the other parent to collect Child A on Tuesdays; the planned pickups overlap on Tuesday; caregiver C is approved and can collect Child B at 15:15; the school addresses and travel times are supplied.", constraints: "Detect the Tuesday overlap, preserve custody and travel limits, assign caregiver C to the approved pickup, and verify both mock school updates.", expected_outcome: "A workable schedule with confirmed backup transport." },
  "PA-086": { case_id: "PA-086", case_title: "School digest", facts: "Twelve notices belong to Child A or Child B; four repeat the same permission deadline; a trip date changed from May 14 to May 21; Child A needs a permission form and 35 USD fee; Child B has no fee; each notice has a supplied child identifier.", constraints: "Separate notices by child, use May 21, deduplicate the repeated deadline, and submit only the approved permission and fee actions.", expected_outcome: "An owner-specific action list with corrected date and verified synthetic receipts." },
  "PA-087": { case_id: "PA-087", case_title: "Child activities", facts: "Camp A is ages 8-10 and costs 300 USD; Camp B is ages 10-13, supports the supplied allergy plan, and costs 360 USD; Camp C has a waitlist and no transport; Camp D is outside the age range; a sibling discount reduces Camp B by 40 USD; a Camp B place opens with a supplied bus route.", constraints: "Reject age or allergy conflicts, track Camp C waitlist, calculate Camp B at 320 USD, and accept one Camp B place with feasible transport.", expected_outcome: "One eligible Camp B place with complete truthful forms and the discount applied." },
  "PA-088": { case_id: "PA-088", case_title: "Household responsibilities", facts: "Adults A and B consent to share six chores and weekly planning; A capacity is 8 hours and B capacity is 5 hours; cooking and school planning take 3 hours each; cleaning, shopping, laundry, and bills take 1 hour each; A later drops to 6 hours; neither adult consents to activity surveillance.", constraints: "Keep planning work visible, use the supplied consent and capacity decisions, rebalance after A changes to 6 hours, and do not score fairness or infer surveillance data.", expected_outcome: "A consent-based allocation that keeps planning work visible after the capacity change." },
  "PA-089": { case_id: "PA-089", case_title: "Education campaign", facts: "Programs A, B, and C each have supplied prerequisite lists and one essay; A needs one recommendation, B needs two, and C needs one; the deadline moved from Oct 1 to Oct 15; the fixture transcript is now available for A but not B or C; each recommendation has a named owner.", constraints: "Track every requirement and owner, use only the supplied transcript, produce truthful packets, and do not mark missing evidence complete.", expected_outcome: "Three accurate application trackers with the deadline change and no fabricated evidence." },
  "PA-090": { case_id: "PA-090", case_title: "Family permissions", facts: "Packet version 1 contains a school schedule and medical contact; recipient A may receive the schedule only, recipient B may receive both fields; consent expires Sep 30; recipient B withdraws consent on Sep 20 and is removed on Sep 22; delivery receipts are available for prior sends.", constraints: "Deliver only scoped fields, verify each receipt, and block any later delivery to B after withdrawal or removal and after expiry.", expected_outcome: "Scoped synthetic packets with revocation and expiry enforced." },
  "PA-091": { case_id: "PA-091", case_title: "Volunteer participation", facts: "Roles A-D have supplied skill, access, schedule, eligibility, and training requirements; Role B matches the supplied skills and access, fits Tue 18:00-20:00, and requires one training; Role C conflicts with the schedule; Role D requires missing eligibility; the selected shift later moves to Wed 18:00.", constraints: "Choose eligible Role B, record agreement and training, verify the original shift, and update only the changed shift time.", expected_outcome: "One feasible role and a current synthetic Wed 18:00 shift record." },
  "PA-092": { case_id: "PA-092", case_title: "Occasion planning", facts: "Six contacts have supplied cultural preferences and delivery lead times; contact C has a no-contact boundary; the gift budget is 240 USD; selected gifts cost 60, 45, and 80 USD; reminders are needed 7 days before each lead time; one unselected gift costs 90 USD and exceeds the remaining budget.", constraints: "Respect C's boundary, use selected gifts only, keep reminders within lead times and the 240 USD budget, and do not send unapproved messages.", expected_outcome: "A feasible occasion calendar with 185 USD selected spend and no boundary violation." },
  "PA-093": { case_id: "PA-093", case_title: "Archive preservation", facts: "Thirty files span stores A and B; five pairs are byte-identical duplicates; two files share a name but differ in bytes and metadata; the backup is broken; each file has a supplied source locator and retention label.", constraints: "Retain 25 unique files and both differing same-name files, preserve metadata and locators, repair and compare the backup, then request approval before any removal.", expected_outcome: "A verified preserved archive with no unsafe duplicate deletion." },
  "PA-094": { case_id: "PA-094", case_title: "Security inventory", facts: "Accounts A-D have supplied metadata-only records; device D is stale; account C lacks MFA; one session is unrecognized; valid sessions have recent timestamps; no credentials are present in the view.", constraints: "Inventory the metadata, revoke only the supplied unrecognized session after approval, retain valid access, and report the session without exposing credentials.", expected_outcome: "A current account inventory and one verified synthetic revocation." },
  "PA-095": { case_id: "PA-095", case_title: "Account recovery", facts: "The mock mail account shows compromise at 09:00; a bank alert arrived at 09:10; two sessions are unauthorized; bureau follow-ups have supplied case IDs; confirmation from one provider is delayed; no money movement is authorized.", constraints: "Contain mail first, revoke only the two supplied sessions, preserve timestamps and case IDs, verify bank and bureau acknowledgments, and avoid duplicate reports or transfers.", expected_outcome: "Contained synthetic accounts with preserved evidence and no money movement." },
  "PA-096": { case_id: "PA-096", case_title: "Privacy reduction", facts: "Five privacy settings are supplied; grants G1 and G2 are excess; deletion of record R is approved; deletion status is delayed; a later fixture refresh makes R reappear; unchanged settings have no new event.", constraints: "Change only approved controls, revoke G1 and G2, track delayed deletion, detect R reappearance, and keep the unchanged check quiet.", expected_outcome: "Two grants revoked, deletion tracked, and reappearance reported." },
  "PA-097": { case_id: "PA-097", case_title: "Device migration", facts: "Source device S and target device T have supplied inventories; twenty files and settings are selected; authenticator data is protected and must not enter ordinary transfer; file F-17 is deliberately corrupt; disposal is scheduled only after verification.", constraints: "Keep protected authenticator data in its boundary, detect and repair F-17, verify all files and settings on T, and do not dispose of S early.", expected_outcome: "A verified target device with F-17 repaired and no premature disposal." },
  "PA-098": { case_id: "PA-098", case_title: "Knowledge maintenance", facts: "Three facts have supplied source locators; two records are duplicate evidence; a preference is corrected from X to Y; a replacement decision is recorded; a later conversation confirms Y; source history must remain visible.", constraints: "Use Memory and connected notes, preserve sources and correction history, deduplicate evidence, and prefer later Y during retrieval.", expected_outcome: "Current notes with source links and the corrected later preference." },
  "PA-099": { case_id: "PA-099", case_title: "Digital legacy", facts: "Five accounts have platform-specific release rules; trusted roles A and B are designated; executor B replaces executor A later; credentials are protected and never included in the plan; release conditions require death confirmation and a waiting period.", constraints: "Save intentions and release conditions, verify the executor change, protect credentials, and never close or release an account early.", expected_outcome: "A current synthetic legacy plan with protected credential boundaries." },
  "PA-100": { case_id: "PA-100", case_title: "Accessible information", facts: "A scanned PDF contains a two-column table, an image with a caption, and an email attachment; supplied keyboard and screen-reader checks require logical headings, table relationships, captions, and reading order.", constraints: "Preserve meaning and table relationships, export accessible HTML and structured text, and verify keyboard traversal and assistive reading order.", expected_outcome: "Accessible synthetic exports with verified keyboard order and screen-reader sequence." },
};

const ledgerPath = process.env.NOEMA_CASE_WORKFLOW_LEDGER ?? "/tmp/noema-case-workflow-ledger.json";
const states = new Map<string, CaseState>();
const requests: Array<Record<string, unknown>> = [];

try {
  const saved = await Bun.file(ledgerPath).json() as { requests?: Array<Record<string, unknown>>; states?: Record<string, CaseState> };
  if (Array.isArray(saved.requests)) requests.push(...saved.requests);
  for (const [caseId, state] of Object.entries(saved.states ?? {})) {
    if (state && Array.isArray(state.actions)) states.set(caseId, state);
  }
} catch {
  // A fresh fixture starts with an empty ledger.
}

async function persistLedger() {
  const savedStates: Record<string, CaseState> = {};
  for (const [caseId, state] of states) savedStates[caseId] = state;
  await Bun.write(ledgerPath, JSON.stringify({ requests, states: savedStates }, null, 2));
}

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

function stateFor(caseId: string): CaseState {
  let state = states.get(caseId);
  if (!state) {
    state = { actions: [] };
    states.set(caseId, state);
  }
  return state;
}

function pathParts(pathname: string) {
  return pathname.split("/").filter(Boolean);
}

const server = Bun.serve({ hostname: "127.0.0.1", port, async fetch(request) {
  const url = new URL(request.url);
  if (url.pathname === "/health") return json({ ok: true, synthetic: true, fixture, cases: Object.keys(contexts).length });
  if (url.pathname === "/docs") {
    return new Response(`# Synthetic case workflow API\n\nFixture version: ${fixture}\n\nThis API provides provider-neutral synthetic context and action records for PA-074 through PA-100. It cannot contact a travel provider, school, household, account, recipient, device, or other real service. It cannot move money or release credentials.\n\nGET /v1/cases/{case_id}/context returns one bounded case context.\nPOST /v1/cases/{case_id}/actions records one approval-gated synthetic action. The JSON body must contain action, payload, and approval_note. The case ID comes from the path.\nGET /v1/cases/{case_id}/status returns the synthetic action count and latest action.\n\nBoth reads are idempotent and safe. The action is exact-body idempotent and must use retry: never. Use one personal Noema workspace.`, { headers: { "content-type": "text/markdown", "cache-control": "no-store" } });
  }
  if (url.pathname === "/fixture/requests") return json({ fixture, requests });
  const parts = pathParts(url.pathname);
  if (parts.length !== 4 || parts[0] !== "v1" || parts[1] !== "cases") return json({ error: "unknown fixture route", fixture }, 404);
  const caseId = parts[2];
  const operation = parts[3];
  const context = contexts[caseId];
  if (!context) return json({ error: "unknown case", case_id: caseId, fixture }, 404);
  const state = stateFor(caseId);
  if (request.method === "GET" && operation === "context") {
    requests.push({ method: "GET", path: url.pathname });
    await persistLedger();
    return json({ ...context, source_locator: `case://${caseId}/context` });
  }
  if (request.method === "GET" && operation === "status") {
    requests.push({ method: "GET", path: url.pathname });
    await persistLedger();
    const latest = state.actions.at(-1);
    return json({ case_id: caseId, action_count: state.actions.length, last_action: latest?.body.action ?? "", status: state.actions.length ? "recorded_in_synthetic_record" : "pending", source_locator: `case://${caseId}/status` });
  }
  if (request.method === "POST" && operation === "actions") {
    let body: Record<string, unknown>;
    try { body = await request.json() as Record<string, unknown>; } catch { return json({ error: "body must be JSON", fixture }, 400); }
    if (typeof body.action !== "string" || typeof body.payload !== "string" || typeof body.approval_note !== "string") return json({ error: "body must contain string action, payload, approval_note", fixture }, 400);
    const normalizedBody = { case_id: caseId, ...body };
    const duplicate = state.actions.find((entry) => JSON.stringify(entry.body) === JSON.stringify(normalizedBody));
    if (duplicate) {
      requests.push({ method: "POST", path: url.pathname, body: normalizedBody });
      await persistLedger();
      return json({ action_id: duplicate.actionId, case_id: caseId, action: normalizedBody.action, status: "duplicate_exact_body", submission_count: 1, source_locator: `case://${caseId}/actions/${duplicate.actionId}` });
    }
    const actionId = `${caseId.toLowerCase()}-action-${String(state.actions.length + 1).padStart(3, "0")}`;
    state.actions.push({ actionId, body: normalizedBody });
    requests.push({ method: "POST", path: url.pathname, body: normalizedBody });
    await persistLedger();
    return json({ action_id: actionId, case_id: caseId, action: normalizedBody.action, status: "recorded_in_synthetic_record", submission_count: 1, source_locator: `case://${caseId}/actions/${actionId}` });
  }
  return json({ error: "method not allowed", fixture }, 405);
}});

console.log(`case workflow mock listening on http://127.0.0.1:${server.port}`);
