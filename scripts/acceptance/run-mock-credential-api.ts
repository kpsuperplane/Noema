#!/usr/bin/env bun

// Synthetic professional-credential service for PA-027.
// It has no licensing authority and cannot contact a real registrar.

const port = Number(process.env.NOEMA_CREDENTIAL_PORT ?? "3741");
const fixtureVersion = "2026-09-08-credential-api-v1";

type Credit = {
  id: string;
  title: string;
  earnedAt: string;
  credits: number;
  status: "VALID" | "DUPLICATE" | "EXPIRED";
  duplicateOf?: string;
  expiresAt?: string;
};

type Course = {
  id: string;
  title: string;
  credits: number;
  format: string;
  startsAt: string;
  completesAt: string;
  eligible: boolean;
  eligibilityNote: string;
};

type Enrollment = {
  id: string;
  courseId: string;
  status: "SCHEDULED" | "COMPLETED";
  evidenceId: string;
  completedAt: string;
};

type Renewal = {
  id: string;
  creditIds: string[];
  enrollmentIds: string[];
  status: "SUBMITTED" | "APPROVED";
  submissionCount: number;
};

const credits: Credit[] = [
  ...Array.from({ length: 8 }, (_, index) => ({
    id: `credit-${String(index + 1).padStart(3, "0")}`,
    title: `Continuing education record ${index + 1}`,
    earnedAt: `2026-0${Math.floor(index / 4) + 1}-${String((index % 4) + 3).padStart(2, "0")}`,
    credits: 1,
    status: "VALID" as const,
  })),
  {
    id: "credit-009",
    title: "Duplicate ethics record",
    earnedAt: "2026-03-11",
    credits: 1,
    status: "DUPLICATE",
    duplicateOf: "credit-003",
  },
  {
    id: "credit-010",
    title: "Duplicate safety record",
    earnedAt: "2026-03-12",
    credits: 1,
    status: "DUPLICATE",
    duplicateOf: "credit-004",
  },
  {
    id: "credit-011",
    title: "Expired law update",
    earnedAt: "2024-02-02",
    credits: 1,
    status: "EXPIRED",
    expiresAt: "2026-02-02",
  },
  {
    id: "credit-012",
    title: "Expired practice update",
    earnedAt: "2024-02-08",
    credits: 1,
    status: "EXPIRED",
    expiresAt: "2026-02-08",
  },
];

const credential = {
  id: "credential-001",
  name: "Synthetic professional credential",
  requiredCredits: 12,
  renewalDeadline: "2026-10-08",
  status: "RENEWAL_REQUIRED",
};

const courses: Course[] = [
  {
    id: "course-privacy-2",
    title: "Privacy practice update",
    credits: 2,
    format: "online evening",
    startsAt: "2026-09-15T18:00:00Z",
    completesAt: "2026-09-15T20:00:00Z",
    eligible: true,
    eligibilityNote: "Counts toward this credential and completes before the renewal deadline.",
  },
  {
    id: "course-safety-2",
    title: "Safety practice update",
    credits: 2,
    format: "online evening",
    startsAt: "2026-09-22T18:00:00Z",
    completesAt: "2026-09-22T20:00:00Z",
    eligible: true,
    eligibilityNote: "Counts toward this credential and completes before the renewal deadline.",
  },
  {
    id: "course-advanced-4",
    title: "Advanced practice workshop",
    credits: 4,
    format: "in person weekend",
    startsAt: "2026-10-20T16:00:00Z",
    completesAt: "2026-10-20T22:00:00Z",
    eligible: false,
    eligibilityNote: "Completes after the renewal deadline.",
  },
];

let enrollments: Enrollment[] = [];
let renewal: Renewal | undefined;

const docs = `# Synthetic credential renewal API

Fixture version: ${fixtureVersion}

This is a deterministic test service. It has no licensing authority and cannot
contact a registrar, course provider, or professional board. All records are
synthetic. The base URL is the URL that served this document.

## Operations

- \`GET /v1/credential\`: return the credential requirement and renewal deadline.
- \`GET /v1/credits\`: return all credit records. Count only \`VALID\` records;
  \`DUPLICATE\` records point to their original and \`EXPIRED\` records do not count.
- \`GET /v1/courses\`: return candidate courses and their eligibility notes.
- \`POST /v1/course-enrollments\`: schedule one course with JSON \`course_id\`.
  Repeating the same course returns its original enrollment. A scheduled course
  is completed by the next enrollment read and exposes a completion evidence ID.
- \`GET /v1/course-enrollments\`: return scheduled courses and completion evidence.
- \`POST /v1/renewals\`: submit one renewal with JSON arrays \`credit_ids\` and
  \`enrollment_ids\`. A repeated submission returns the original renewal.
- \`GET /v1/renewals/{id}\`: read renewal status. A submitted renewal becomes
  \`APPROVED\` on its first status read.

The credential requires 12 credits and expires on 2026-10-08. The fixture has
eight valid credits, two duplicates, and two expired records. The two eligible
two-credit courses provide the four missing credits. The later four-credit
course is not eligible because it completes after the deadline.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

function pathId(path: string, prefix: string) {
  return path.startsWith(prefix) ? path.slice(prefix.length) : undefined;
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "credential", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, { headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" } });
    }
    if (request.method === "GET" && url.pathname === "/v1/credential") {
      return json({
        id: credential.id,
        name: credential.name,
        requiredCredits: credential.requiredCredits,
        renewalDeadline: credential.renewalDeadline,
        required_credits: credential.requiredCredits,
        renewal_deadline: credential.renewalDeadline,
        status: credential.status,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/credits") {
      return json({
        credits: credits.map((entry) => ({
          id: entry.id,
          title: entry.title,
          name: entry.title,
          earnedAt: entry.earnedAt,
          credits: entry.credits,
          creditHours: entry.credits,
          status: entry.status,
          ...(entry.duplicateOf ? { duplicateOf: entry.duplicateOf } : {}),
          ...(entry.expiresAt ? { expiresOn: entry.expiresAt } : {}),
        })),
        next_page_token: null,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/courses") {
      return json({
        courses: courses.map((entry) => ({
          id: entry.id,
          title: entry.title,
          credits: entry.credits,
          format: entry.format,
          startsAt: entry.startsAt,
          completionDate: entry.completesAt,
          name: entry.title,
          creditHours: entry.credits,
          eligibleForRenewal: entry.eligible,
          eligible: entry.eligible,
          eligibilityNote: entry.eligibilityNote,
        })),
        next_page_token: null,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/course-enrollments") {
      enrollments = enrollments.map((entry) => ({ ...entry, status: "COMPLETED" }));
      return json({
        enrollments: enrollments.map((entry) => ({
          id: entry.id,
          enrollmentId: entry.id,
          course_id: entry.courseId,
          courseId: entry.courseId,
          status: entry.status,
          completionEvidenceId: entry.evidenceId,
          completedAt: entry.completedAt,
        })),
        next_page_token: null,
      });
    }
    if (request.method === "POST" && url.pathname === "/v1/course-enrollments") {
      const body = await request.json().catch(() => undefined) as Record<string, unknown> | undefined;
      const courseId = typeof body?.course_id === "string" ? body.course_id : "";
      const course = courses.find((candidate) => candidate.id === courseId);
      if (!course) return json({ error: "unknown_course" }, 404);
      const existing = enrollments.find((entry) => entry.courseId === courseId);
      if (existing) return json({ id: existing.id, enrollmentId: existing.id, course_id: existing.courseId, courseId: existing.courseId, status: existing.status });
      const entry: Enrollment = { id: `enrollment-${String(enrollments.length + 1).padStart(3, "0")}`, courseId, status: "SCHEDULED", evidenceId: `evidence-${courseId}`, completedAt: course.completesAt };
      enrollments = [...enrollments, entry];
      return json({ id: entry.id, enrollmentId: entry.id, course_id: entry.courseId, courseId: entry.courseId, status: entry.status }, 201);
    }
    if (request.method === "POST" && url.pathname === "/v1/renewals") {
      const body = await request.json().catch(() => undefined) as Record<string, unknown> | undefined;
      const creditIds = Array.isArray(body?.credit_ids) ? body.credit_ids.map(String) : undefined;
      const enrollmentIds = Array.isArray(body?.enrollment_ids) ? body.enrollment_ids.map(String) : undefined;
      if (!creditIds || !enrollmentIds) return json({ error: "credit_ids and enrollment_ids are required" }, 400);
      if (renewal) return json({ ...renewal, renewalId: renewal.id });
      renewal = { id: "renewal-001", creditIds, enrollmentIds, status: "SUBMITTED", submissionCount: 1 };
      return json({ ...renewal, renewalId: renewal.id }, 201);
    }
    const renewalId = pathId(url.pathname, "/v1/renewals/");
    if (request.method === "GET" && renewalId) {
      if (!renewal || renewal.id !== renewalId) return json({ error: "not_found" }, 404);
      renewal.status = "APPROVED";
      return json({ ...renewal, renewalId: renewal.id });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic credential API listening on http://127.0.0.1:${server.port}`);
