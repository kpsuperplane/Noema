#!/usr/bin/env bun

// Synthetic course catalogue for PA-040.
// It cannot charge a card, enroll with a real provider, or contact an employer.

const port = Number(process.env.NOEMA_COURSE_PORT ?? "3762");
const fixtureVersion = "2026-09-08-course-choice-api-v1";

type Course = {
  id: string;
  title: string;
  summary: string;
  prerequisites: string;
  prerequisite_status: "met" | "not_met";
  schedule: string;
  time_zone: string;
  duration_hours: number;
  tuition_usd: number;
  materials_usd: number;
  registration_fee_usd: number;
  total_cost_usd: number;
  enrollment_deadline: string;
  goal_fit: string;
};

type Enrollment = {
  id: string;
  course_id: string;
  status: "PENDING" | "CONFIRMED";
  receipt_id: string;
  schedule: string;
  time_zone: string;
  total_cost_usd: number;
  created_at: string;
  submission_count: number;
};

const profile = {
  learner_id: "learner-001",
  learner_name: "Jordan Lee",
  time_zone: "America/New_York",
  work_schedule: "Monday-Friday, 09:00-17:00 America/New_York",
  available_windows: "Tuesday and Thursday, 18:00-20:00; Saturday, 09:00-13:00 America/New_York",
  goal: "Build practical data-automation skills for a current analyst role.",
  budget_usd: 400,
  preferred_format: "live instruction with a small project",
  enrollment_deadline: "2026-09-30",
};

const courses: Course[] = [
  {
    id: "course-luau-automation",
    title: "Luau Automation Foundations",
    summary: "Live project course for small data workflows and repeatable automation.",
    prerequisites: "None",
    prerequisite_status: "met",
    schedule: "Tuesday, 18:00-20:00",
    time_zone: "America/New_York",
    duration_hours: 8,
    tuition_usd: 240,
    materials_usd: 35,
    registration_fee_usd: 15,
    total_cost_usd: 290,
    enrollment_deadline: "2026-09-20",
    goal_fit: "High: directly teaches data automation and includes a practical project.",
  },
  {
    id: "course-data-operations",
    title: "Data Operations for Analysts",
    summary: "Live project course for reliable data checks, handoffs, and reporting.",
    prerequisites: "SQL basics",
    prerequisite_status: "met",
    schedule: "Saturday, 10:00-12:00",
    time_zone: "America/New_York",
    duration_hours: 10,
    tuition_usd: 310,
    materials_usd: 20,
    registration_fee_usd: 0,
    total_cost_usd: 330,
    enrollment_deadline: "2026-09-25",
    goal_fit: "Medium-high: supports the current analyst role and uses a reporting project.",
  },
  {
    id: "course-python-automation",
    title: "Python for Automation",
    summary: "Live weekday course for scripting and workflow automation.",
    prerequisites: "None",
    prerequisite_status: "met",
    schedule: "Thursday, 10:00-12:00",
    time_zone: "America/New_York",
    duration_hours: 8,
    tuition_usd: 180,
    materials_usd: 25,
    registration_fee_usd: 25,
    total_cost_usd: 230,
    enrollment_deadline: "2026-09-18",
    goal_fit: "High: automation content fits the stated goal.",
  },
  {
    id: "course-cloud-data-lab",
    title: "Cloud Data Lab",
    summary: "Long live lab for cloud pipelines and deployment practice.",
    prerequisites: "Cloud fundamentals",
    prerequisite_status: "not_met",
    schedule: "Tuesday, 18:00-21:00",
    time_zone: "America/Los_Angeles",
    duration_hours: 12,
    tuition_usd: 200,
    materials_usd: 50,
    registration_fee_usd: 75,
    total_cost_usd: 325,
    enrollment_deadline: "2026-09-12",
    goal_fit: "High: useful cloud extension, but the prerequisite is not complete.",
  },
];

let enrollment: Enrollment | undefined;
const requests: Array<{ method: string; path: string; body?: unknown }> = [];

const docs = `# Synthetic course catalogue API

Fixture version: ${fixtureVersion}

This deterministic service is synthetic. It cannot charge a card, contact an
employer, or enroll anyone with a real school. The base URL is the URL that
served this document. All money values are US dollars.

## Profile

- GET /v1/profile: return JSON fields time_zone, work_schedule,
  available_windows, learning_goal, budget, preferred_format, and
  enrollment_deadline for the learner.

## Catalogue

- GET /v1/courses: return a JSON array of four course records. Each record
  includes title, course_id, prerequisite, status, meeting_schedule,
  time_zone, duration, tuition, materials_cost, registration_fee,
  total_cost, enrollment_deadline, and goal_fit. The total cost is
  tuition plus materials plus registration fee.
- The learner is available only Tuesday or Thursday from 18:00 to 20:00 and
  Saturday from 09:00 to 13:00 in America/New_York. A course must also have a
  met prerequisite and a deadline no later than 2026-09-30.

## Enrollment

- POST /v1/enrollments: create one synthetic enrollment. The JSON body must
  contain exactly one course_id. The response includes enrollment_id,
  course_id, and submission_count. A repeated request for the same
  course returns the original enrollment and keeps submission_count at 1.
- GET /v1/enrollments/{enrollment_id}: return enrollment_id,
  selected_course, confirmed_schedule, time_zone, total_cost, and
  status. Reading a pending enrollment changes only its synthetic
  status to CONFIRMED.

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
    requests.push({ method: request.method, path: `${url.pathname}${url.search}`, ...(body === undefined ? {} : { body }) });
    console.log(JSON.stringify(requests.at(-1)));

    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "course-choice", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, { headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" } });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json({
        time_zone: profile.time_zone,
        work_schedule: profile.work_schedule,
        available_windows: profile.available_windows,
        learning_goal: profile.goal,
        budget: profile.budget_usd,
        preferred_format: profile.preferred_format,
        enrollment_deadline: profile.enrollment_deadline,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/courses") {
      return json(courses.map((course) => ({
        title: course.title,
        course_id: course.id,
        prerequisite: course.prerequisites,
        status: course.prerequisite_status,
        meeting_schedule: course.schedule,
        time_zone: course.time_zone,
        duration: `${course.duration_hours} hours`,
        tuition: course.tuition_usd,
        materials_cost: course.materials_usd,
        registration_fee: course.registration_fee_usd,
        total_cost: course.total_cost_usd,
        enrollment_deadline: course.enrollment_deadline,
        goal_fit: course.goal_fit,
      })));
    }
    if (request.method === "POST" && url.pathname === "/v1/enrollments") {
      const courseId = typeof (body as Record<string, unknown> | undefined)?.course_id === "string"
        ? (body as Record<string, string>).course_id
        : "";
      const course = courses.find((candidate) => candidate.id === courseId);
      if (!course) return json({ error: "unknown_course" }, 404);
      if (enrollment) {
        return json({
          enrollment_id: enrollment.id,
          course_id: enrollment.course_id,
          submission_count: enrollment.submission_count,
        });
      }
      enrollment = {
        id: "enrollment-001",
        course_id: course.id,
        status: "PENDING",
        receipt_id: "receipt-course-001",
        schedule: course.schedule,
        time_zone: course.time_zone,
        total_cost_usd: course.total_cost_usd,
        created_at: "2026-09-08T12:00:00Z",
        submission_count: 1,
      };
      return json({
        enrollment_id: enrollment.id,
        course_id: enrollment.course_id,
        submission_count: enrollment.submission_count,
      }, 201);
    }
    const enrollmentId = pathId(url.pathname, "/v1/enrollments/");
    if (request.method === "GET" && enrollmentId) {
      if (!enrollment || enrollment.id !== enrollmentId) return json({ error: "not_found" }, 404);
      enrollment = { ...enrollment, status: "CONFIRMED" };
      return json({
        enrollment_id: enrollment.id,
        selected_course: enrollment.course_id,
        confirmed_schedule: enrollment.schedule,
        time_zone: enrollment.time_zone,
        total_cost: enrollment.total_cost_usd,
        status: enrollment.status,
      });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic course-choice API listening on http://127.0.0.1:${server.port}`);
