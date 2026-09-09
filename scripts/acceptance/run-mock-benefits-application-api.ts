#!/usr/bin/env bun

// Synthetic benefits-application service for PA-046.
// It cannot submit to a government program or expose a real household.

const port = Number(process.env.NOEMA_BENEFITS_PORT ?? "3768");
const fixtureVersion = "2026-09-09-benefits-application-api-v1";

type Application = {
  application_id: string;
  receipt_id: string;
  household_id: string;
  program_id: string;
  document_ids: string[];
  status: "submitted";
  submission_count: number;
  submitted_at: string;
  review_window_days: number;
  renewal_date: string;
  reporting_duties: string[];
  source_locator: string;
};

const profile = {
  account_id: "benefits-household-001",
  household_label: "Jordan Lee household (synthetic)",
  household_size: 3,
  annual_income: 58400,
  currency: "USD",
  as_of: "2026-09-09",
  program_id: "family-support-2026",
  program_name: "Family Support Benefit (synthetic)",
  review_goal: "Prepare a benefits prescreen and track the next renewal.",
  renewal_date: "2026-12-01",
  source_locator: "benefits://accounts/benefits-household-001/profile",
};

const programs = [
  {
    program_id: "family-support-2026",
    program_name: "Family Support Benefit (synthetic)",
    household_size: 3,
    annual_income_threshold: 72000,
    currency: "USD",
    screening_result: "income_within_threshold",
    official_determination: false,
    source_locator: "benefits://programs/family-support-2026",
  },
];

const requirements = [
  {
    requirement_id: "identity",
    label: "Household identity document",
    status: "received",
    source_locator: "benefits://requirements/identity",
  },
  {
    requirement_id: "residency",
    label: "Residency document",
    status: "received",
    source_locator: "benefits://requirements/residency",
  },
  {
    requirement_id: "wage_statement",
    label: "Current wage statement",
    status: "missing",
    source_locator: "benefits://requirements/wage_statement",
  },
];

const documents = [
  {
    document_id: "identity-household-001",
    document_type: "household_identity",
    status: "received",
    source_locator: "benefits://documents/identity-household-001",
  },
  {
    document_id: "residency-household-001",
    document_type: "residency",
    status: "received",
    source_locator: "benefits://documents/residency-household-001",
  },
];

const missingWageStatement = {
  document_id: "wage-statement-2026-001",
  document_type: "current_wage_statement",
  employer: "Northstar Analytics LLC (synthetic)",
  covered_period: "2026-08-16/2026-08-31",
  annualized_income: 58400,
  currency: "USD",
  status: "received",
  source_locator: "benefits://documents/wage-statement-2026-001",
};

const reportingDuties = [
  "Report a household-size or income change within 10 days.",
  "Keep current income records for the renewal review.",
];

let application: Application | undefined;
const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const docs = `# Synthetic benefits-application API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot determine real eligibility,
submit to a government program, contact an agency, or provide benefits. All
amounts and people are test records in US dollars.

## Operations

- GET /v1/profile returns account, household size, annual income, program, and
  renewal context.
- GET /v1/programs returns the program's income threshold and a prescreening
  result. The result is not an official eligibility determination.
- GET /v1/requirements returns received and missing document requirements.
- GET /v1/documents returns the documents already held by the household.
- GET /v1/documents/missing-wage-statement returns the missing fixture wage
  statement. Retrieving it is a read-only collection step.
- POST /v1/applications records one mock application. Its JSON body must have
  program_id, household_id, and document_ids. The document list must include
  the fixture wage statement. A repeated submission returns the same receipt
  and does not increase submission_count. This route requires human approval
  in the client and never contacts a benefits agency.
- GET /v1/applications/{application_id} returns the mock receipt, status,
  reporting duties, and renewal date.
- GET /v1/renewal returns the renewal date, reporting duties, and checklist.

The reporting duties are fixture guidance, not legal advice. The service has
no payment, agency-contact, appeal, or real application endpoint.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function pathId(path: string, prefix: string) {
  return path.startsWith(prefix) ? path.slice(prefix.length) : undefined;
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    let body: unknown;
    if (request.method !== "GET") {
      body = await request.json().catch(() => undefined);
    }
    requests.push({
      method: request.method,
      path: `${url.pathname}${url.search}`,
      ...(body === undefined ? {} : { body }),
    });
    console.log(JSON.stringify(requests.at(-1)));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "benefits-application", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/programs") return json({ programs });
    if (request.method === "GET" && url.pathname === "/v1/requirements") {
      return json({ requirements, reporting_duties: reportingDuties });
    }
    if (request.method === "GET" && url.pathname === "/v1/documents") return json({ documents });
    if (request.method === "GET" && url.pathname === "/v1/documents/missing-wage-statement") {
      return json(missingWageStatement);
    }
    if (request.method === "POST" && url.pathname === "/v1/applications") {
      const value = (body ?? {}) as Record<string, unknown>;
      const documentIds = Array.isArray(value.document_ids) ? value.document_ids.map(String) : [];
      if (value.program_id !== profile.program_id || value.household_id !== profile.account_id ||
          !documentIds.includes(missingWageStatement.document_id)) {
        return json({ error: "program_id, household_id, and the missing wage statement are required" }, 400);
      }
      if (application) return json(application);
      application = {
        application_id: "application-family-support-001",
        receipt_id: "receipt-family-support-001",
        household_id: profile.account_id,
        program_id: profile.program_id,
        document_ids: documentIds,
        status: "submitted",
        submission_count: 1,
        submitted_at: "2026-09-09T00:00:00Z",
        review_window_days: 14,
        renewal_date: profile.renewal_date,
        reporting_duties: reportingDuties,
        source_locator: "benefits://applications/application-family-support-001",
      };
      return json(application, 201);
    }
    const applicationId = pathId(url.pathname, "/v1/applications/");
    if (request.method === "GET" && applicationId) {
      if (!application || application.application_id !== applicationId) return json({ error: "not_found" }, 404);
      return json(application);
    }
    if (request.method === "GET" && url.pathname === "/v1/renewal") {
      return json({
        renewal_date: profile.renewal_date,
        status: application ? "application_submitted" : "prescreen",
        reporting_duties: reportingDuties,
        checklist: ["Keep current income records", "Review household changes", "Renew by the documented date"],
        source_locator: "benefits://accounts/benefits-household-001/renewal",
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic benefits-application API listening on http://127.0.0.1:${server.port}`);
