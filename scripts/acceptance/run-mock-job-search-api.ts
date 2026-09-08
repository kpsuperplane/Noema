#!/usr/bin/env bun

// Synthetic job-search service for PA-028.
// It has no connection to a job board, employer, recruiter, or mail system.

const port = Number(process.env.NOEMA_JOB_SEARCH_PORT ?? "3746");
const fixtureVersion = "2026-09-08-job-search-api-v1";

type Job = {
  id: string;
  title: string;
  company: string;
  location: string;
  workMode: "remote" | "hybrid" | "onsite";
  applicationDeadline: string;
  previousDeadline?: string;
  deadlineChanged: boolean;
  sourceUrl: string;
};

type Application = {
  id: string;
  jobId: string;
  stage: "APPLIED" | "SCREENING" | "INTERVIEW";
  nextAction: string;
  nextActionDue: string;
};

type FollowUp = {
  id: string;
  applicationId: string;
  message: string;
  status: "SENT";
  receiptId: string;
  submissionCount: number;
};

const profile = {
  preferredLocations: ["San Francisco", "Remote US"],
  allowedWorkModes: ["remote", "hybrid"],
  locationConstraint: "San Francisco or fully remote within the United States",
};

const jobs: Job[] = [
  {
    id: "job-001",
    title: "Senior Product Engineer",
    company: "Acme Systems",
    location: "San Francisco, CA",
    workMode: "hybrid",
    applicationDeadline: "2026-09-20",
    deadlineChanged: false,
    sourceUrl: "https://jobs.example.test/acme/job-001",
  },
  {
    id: "job-002",
    title: "Product Operations Lead",
    company: "Beacon Labs",
    location: "Remote US",
    workMode: "remote",
    applicationDeadline: "2026-09-18",
    previousDeadline: "2026-09-10",
    deadlineChanged: true,
    sourceUrl: "https://jobs.example.test/beacon/job-002",
  },
  {
    id: "job-003",
    title: "Research Program Manager",
    company: "Cinder Research",
    location: "New York, NY",
    workMode: "onsite",
    applicationDeadline: "2026-09-25",
    deadlineChanged: false,
    sourceUrl: "https://jobs.example.test/cinder/job-003",
  },
  {
    id: "job-004",
    title: "Data Analyst",
    company: "Delta Commerce",
    location: "Austin, TX",
    workMode: "onsite",
    applicationDeadline: "2026-09-28",
    deadlineChanged: false,
    sourceUrl: "https://jobs.example.test/delta/job-004",
  },
  {
    id: "job-005",
    title: "Customer Success Manager",
    company: "Ember Cloud",
    location: "Remote US",
    workMode: "remote",
    applicationDeadline: "2026-09-30",
    deadlineChanged: false,
    sourceUrl: "https://jobs.example.test/ember/job-005",
  },
];

const applications: Application[] = [
  {
    id: "application-001",
    jobId: "job-001",
    stage: "SCREENING",
    nextAction: "Prepare for the technical screening",
    nextActionDue: "2026-09-12",
  },
  {
    id: "application-002",
    jobId: "job-002",
    stage: "APPLIED",
    nextAction: "Follow up with the recruiter",
    nextActionDue: "2026-09-14",
  },
  {
    id: "application-003",
    jobId: "job-005",
    stage: "INTERVIEW",
    nextAction: "Send a thank-you note after the interview",
    nextActionDue: "2026-09-10",
  },
];

const followUps: FollowUp[] = [];

const docs = `# Synthetic job-search API

Fixture version: ${fixtureVersion}

This is a deterministic test service. It has no connection to a job board,
employer, recruiter, email provider, or hiring system. Every record and every
write is synthetic. The base URL is the URL that served this document.

The connected person's location constraint is **San Francisco or fully remote
within the United States**. Hybrid San Francisco roles are allowed. Onsite
roles outside San Francisco are not suitable.

## Operations

- \`GET /v1/profile\`: return the person's job-search location and work-mode
  constraints.
- \`GET /v1/jobs\`: return exactly five tracked jobs. Two are unsuitable
  because they are onsite outside San Francisco. The Beacon Labs job has a
  changed deadline: it moved from 2026-09-10 to 2026-09-18.
- \`GET /v1/applications\`: return exactly three applications. Keep each
  application's stage and next action with its job ID.
- \`PATCH /v1/jobs/{id}\`: update one tracked job's application deadline. The
  JSON body must contain \`application_deadline\`. The operation is safe to
  repeat with the same value and returns the complete updated job.
- \`POST /v1/follow-ups\`: send one synthetic follow-up for an application.
  The JSON body must contain \`application_id\` and \`message\`. Repeating a
  follow-up for the same application returns the original receipt and does
  not create a duplicate.
- \`GET /v1/follow-ups/{id}\`: read the synthetic follow-up receipt.

Responses use JSON. The service returns both snake_case and camelCase aliases
for changed fields so clients can preserve the documented meaning while using
their preferred naming convention.
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

function jobResponse(job: Job) {
  return {
    id: job.id,
    title: job.title,
    company: job.company,
    location: job.location,
    work_mode: job.workMode,
    workMode: job.workMode,
    application_deadline: job.applicationDeadline,
    applicationDeadline: job.applicationDeadline,
    ...(job.previousDeadline
      ? {
          previous_deadline: job.previousDeadline,
          previousDeadline: job.previousDeadline,
        }
      : {}),
    deadline_changed: job.deadlineChanged,
    deadlineChanged: job.deadlineChanged,
    source_url: job.sourceUrl,
    sourceUrl: job.sourceUrl,
  };
}

function applicationResponse(application: Application) {
  return {
    id: application.id,
    job_id: application.jobId,
    jobId: application.jobId,
    stage: application.stage,
    next_action: application.nextAction,
    nextAction: application.nextAction,
    next_action_due: application.nextActionDue,
    nextActionDue: application.nextActionDue,
  };
}

function followUpResponse(followUp: FollowUp) {
  return {
    id: followUp.id,
    application_id: followUp.applicationId,
    applicationId: followUp.applicationId,
    message: followUp.message,
    status: followUp.status,
    receipt_id: followUp.receiptId,
    receiptId: followUp.receiptId,
    submission_count: followUp.submissionCount,
    submissionCount: followUp.submissionCount,
  };
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "job-search", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json({
        preferred_locations: profile.preferredLocations,
        preferredLocations: profile.preferredLocations,
        allowed_work_modes: profile.allowedWorkModes,
        allowedWorkModes: profile.allowedWorkModes,
        location_constraint: profile.locationConstraint,
        locationConstraint: profile.locationConstraint,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/jobs") {
      return json({ jobs: jobs.map(jobResponse), next_page_token: null, nextPageToken: null });
    }
    if (request.method === "GET" && url.pathname === "/v1/applications") {
      return json({ applications: applications.map(applicationResponse), next_page_token: null, nextPageToken: null });
    }

    const jobId = pathId(url.pathname, "/v1/jobs/");
    if (request.method === "PATCH" && jobId) {
      const job = jobs.find((entry) => entry.id === jobId);
      if (!job) return json({ error: "not_found" }, 404);
      const body = (await request.json().catch(() => undefined)) as Record<string, unknown> | undefined;
      const deadline = body?.application_deadline ?? body?.applicationDeadline;
      if (typeof deadline !== "string" || deadline.length === 0) {
        return json({ error: "application_deadline is required" }, 400);
      }
      job.applicationDeadline = deadline;
      return json(jobResponse(job));
    }

    if (request.method === "POST" && url.pathname === "/v1/follow-ups") {
      const body = (await request.json().catch(() => undefined)) as Record<string, unknown> | undefined;
      const applicationId = body?.application_id ?? body?.applicationId;
      const message = body?.message;
      if (typeof applicationId !== "string" || typeof message !== "string" || message.length === 0) {
        return json({ error: "application_id and message are required" }, 400);
      }
      if (!applications.some((entry) => entry.id === applicationId)) {
        return json({ error: "application_not_found" }, 404);
      }
      const existing = followUps.find((entry) => entry.applicationId === applicationId);
      if (existing) return json(followUpResponse(existing));
      const followUp: FollowUp = {
        id: `follow-up-${String(followUps.length + 1).padStart(3, "0")}`,
        applicationId,
        message,
        status: "SENT",
        receiptId: `receipt-follow-up-${String(followUps.length + 1).padStart(3, "0")}`,
        submissionCount: 1,
      };
      followUps.push(followUp);
      return json(followUpResponse(followUp), 201);
    }

    const followUpId = pathId(url.pathname, "/v1/follow-ups/");
    if (request.method === "GET" && followUpId) {
      const followUp = followUps.find((entry) => entry.id === followUpId);
      if (!followUp) return json({ error: "not_found" }, 404);
      return json(followUpResponse(followUp));
    }

    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic job-search API listening on http://127.0.0.1:${server.port}`);
