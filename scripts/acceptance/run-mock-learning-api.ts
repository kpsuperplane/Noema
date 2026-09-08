#!/usr/bin/env bun

// Synthetic learning service for PA-039.
// It has no course account, enrollment, payment, or grading-system write.

const port = Number(process.env.NOEMA_LEARNING_PORT ?? "3761");
const fixtureVersion = "2026-09-08-learning-api-v1";

type Snapshot = "initial" | "followup" | "repeat";

type Week = {
  week: number;
  module: string;
  required_minutes: number;
  optional_minutes: number;
  status: string;
};

type Session = {
  session_id: string;
  week: number;
  minutes: number;
  status: string;
};

const profile = {
  learner_name: "Jordan Lee",
  course_name: "Luau for Data Work",
  plan_id: "luau-data-foundations-2026",
  weeks_total: 8,
  current_week: 3,
  weekly_time_budget_minutes: 180,
  session_length_minutes: 60,
  preferred_days: ["Tuesday", "Thursday", "Saturday"],
  prerequisite_rule:
    "The Module 1 prerequisite quiz must reach 70 before Module 2 work is scheduled.",
  missed_session_rule:
    "Reschedule missed sessions in optional blocks or by moving optional work. Never exceed 180 minutes in one week.",
  pacing_rule:
    "Use prerequisite reinforcement after a failed quiz. Accelerate only after the prerequisite passes and catch-up work is complete.",
};

const baselinePlan: Week[] = [
  { week: 1, module: "Module 1: tables", required_minutes: 120, optional_minutes: 60, status: "complete" },
  { week: 2, module: "Module 1: loops", required_minutes: 120, optional_minutes: 60, status: "complete" },
  { week: 3, module: "Module 1: review and prerequisite retry", required_minutes: 120, optional_minutes: 60, status: "in_progress" },
  { week: 4, module: "Module 2: functions", required_minutes: 120, optional_minutes: 60, status: "planned" },
  { week: 5, module: "Module 2: functions", required_minutes: 120, optional_minutes: 60, status: "planned" },
  { week: 6, module: "Module 3: files", required_minutes: 120, optional_minutes: 60, status: "planned" },
  { week: 7, module: "Module 3: files", required_minutes: 120, optional_minutes: 60, status: "planned" },
  { week: 8, module: "Capstone", required_minutes: 180, optional_minutes: 0, status: "planned" },
];

const initialProgress = {
  snapshot: "initial" as Snapshot,
  current_week: 3,
  prerequisite_quiz: {
    quiz_id: "module-1-prerequisite",
    module: "Module 1",
    score: 55,
    passing_score: 70,
    status: "failed",
  },
  missed_sessions: [
    { session_id: "w3-tue", week: 3, minutes: 60, status: "missed", reason: "work conflict" },
    { session_id: "w3-thu", week: 3, minutes: 60, status: "missed", reason: "illness" },
  ],
  completed_catch_up: [] as Session[],
  completed_sessions: 6,
  available_minutes_this_week: 60,
  next_deadline: "2026-11-01",
  notes: "One Saturday session remains in week 3. Optional blocks in later weeks can hold catch-up work.",
};

const followupProgress = {
  snapshot: "followup" as Snapshot,
  current_week: 5,
  prerequisite_quiz: {
    quiz_id: "module-1-prerequisite-retry",
    module: "Module 1",
    score: 82,
    passing_score: 70,
    status: "passed",
  },
  missed_sessions: [] as Session[],
  completed_catch_up: [
    { session_id: "w3-tue", completed_week: 4, minutes: 60 },
    { session_id: "w3-thu", completed_week: 5, minutes: 60 },
  ],
  completed_sessions: 10,
  available_minutes_this_week: 60,
  next_deadline: "2026-11-01",
  notes: "The prerequisite passed and both missed sessions were completed in optional blocks.",
};

const baseline = { plan_id: profile.plan_id, weeks: baselinePlan };

const docs = `# Synthetic learning API

Fixture version: ${fixtureVersion}

This deterministic, read-only service represents one learner's eight-week
course plan. It has no enrollment, account, payment, grading-system, or real
student data.

## Profile

- GET /v1/profile: return the learner, course, eight-week plan, preferred
  session days, 180-minute weekly limit, and the prerequisite, missed-session,
  and pacing rules.
- The Module 1 prerequisite quiz must score at least 70 before Module 2 work
  can be scheduled.
- Every week's total is required minutes plus optional minutes. A revised plan
  may use an optional block for catch-up, but must stay at or below 180 total
  minutes and may move optional work to a later week.

## Plan

- GET /v1/plan: return the current eight-week plan. Each week has
  week, module, required_minutes, optional_minutes, and status.
- The baseline has two missed 60-minute sessions in week 3. The remaining
  Saturday slot is 60 minutes. Weeks 4 through 7 each have one 60-minute
  optional block that can hold one catch-up session.

## Progress snapshots

- GET /v1/progress?snapshot=initial: return a failed score of 55, a passing
  score of 70, two missed 60-minute sessions, zero completed catch-up sessions,
  and one 60-minute slot left in week 3.
- GET /v1/progress?snapshot=followup: return a retry score of 82, zero missed
  sessions, both catch-up sessions completed in optional blocks, and current
  week 5.
- GET /v1/progress?snapshot=repeat: return the exact same values as the
  follow-up snapshot. It represents an unchanged check and must not create a
  new plan revision.

The useful initial adjustment is to use the remaining week-3 slot for
prerequisite review, keep Module 2 locked, and place the two missed sessions in
week-4 and week-5 optional blocks. After the follow-up passes, Module 2 may
start in the current week and later optional work may move earlier. The weekly
total must remain at or below 180 minutes in every revised week.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

function progressFor(snapshot: Snapshot) {
  if (snapshot === "initial") return initialProgress;
  return { ...followupProgress, snapshot };
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "learning", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") return json(profile);
    if (request.method === "GET" && url.pathname === "/v1/plan") return json(baseline);
    if (request.method === "GET" && url.pathname === "/v1/progress") {
      const snapshot = url.searchParams.get("snapshot");
      if (snapshot === "initial" || snapshot === "followup" || snapshot === "repeat") {
        return json(progressFor(snapshot));
      }
      return json({ error: "snapshot must be initial, followup, or repeat" }, 400);
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic learning API listening on http://127.0.0.1:${server.port}`);
