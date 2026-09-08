#!/usr/bin/env bun

// Synthetic literature-index service for PA-034.
// It has no connection to a publisher, index, researcher, or real account.

const port = Number(process.env.NOEMA_LITERATURE_PORT ?? "3755");
const fixtureVersion = "2026-09-08-literature-api-v1";

type Paper = {
  recordId: string;
  workId: string;
  title: string;
  authors: string[];
  year: number;
  doi: string;
  studyDesign: string;
  population: string;
  finding: string;
  limitation: string;
  status: "published" | "retracted";
  retractionNotice?: string;
  sourceUrl: string;
};

const profile = {
  question: "Does a four-day workweek improve employee wellbeing without reducing service output?",
  scope: "Peer-reviewed research on four-day workweeks, employee wellbeing, and measured output.",
  search_bounds: {
    indexes: ["Northstar Index", "Harbor Index"],
    index_a_endpoint: "/v1/index-a/search",
    index_b_endpoint: "/v1/index-b/search",
    max_records_per_index: 6,
    searched_on: "2026-09-08",
  },
  review_preferences: [
    "Deduplicate records that identify the same work by work_id or DOI.",
    "Mark retracted work and do not use it as current evidence.",
    "Compare study designs, populations, findings, and limitations.",
    "Keep conflicting results and state gaps instead of forcing consensus.",
  ],
};

const indexA: Paper[] = [
  {
    recordId: "northstar-001",
    workId: "work-001",
    title: "A Four-Day Week Trial in Software Teams",
    authors: ["Mira Chen", "Jon Bell"],
    year: 2024,
    doi: "10.5555/fdw.001",
    studyDesign: "Randomized controlled trial",
    population: "312 software employees in 24 teams",
    finding: "Wellbeing scores improved and measured sprint output did not decline over six months.",
    limitation: "The trial was voluntary and limited to software teams with flexible work practices.",
    status: "published",
    sourceUrl: "https://literature.example.test/northstar/work-001",
  },
  {
    recordId: "northstar-002",
    workId: "work-002",
    title: "Reduced Hours and Customer-Service Throughput",
    authors: ["Luis Ortega", "Priya Shah"],
    year: 2023,
    doi: "10.5555/fdw.002",
    studyDesign: "Prospective cohort study",
    population: "18 customer-service centers followed for nine months",
    finding: "Burnout fell, but average ticket throughput was 4% lower after hours were reduced.",
    limitation: "Centers selected their own schedules and used different staffing ratios.",
    status: "published",
    sourceUrl: "https://literature.example.test/northstar/work-002",
  },
  {
    recordId: "northstar-003",
    workId: "work-003",
    title: "What Workers Say About Four-Day Schedules",
    authors: ["Asha Green", "Ravi Nair"],
    year: 2022,
    doi: "10.5555/fdw.003",
    studyDesign: "Qualitative interview study",
    population: "64 employees and 19 managers in public agencies",
    finding: "Workers described better recovery; managers reported handoff and coverage strain.",
    limitation: "Interview accounts do not measure causal changes in output.",
    status: "published",
    sourceUrl: "https://literature.example.test/northstar/work-003",
  },
  {
    recordId: "northstar-004",
    workId: "work-004",
    title: "Compressed Workweeks in Public Clinics",
    authors: ["Elena Rossi", "David Kim"],
    year: 2025,
    doi: "10.5555/fdw.004",
    studyDesign: "Cluster randomized trial",
    population: "40 outpatient clinics and 1,120 clinical staff",
    finding: "Staff wellbeing improved while appointment completion remained statistically unchanged.",
    limitation: "The nine-month trial did not measure long-term retention or patient outcomes.",
    status: "published",
    sourceUrl: "https://literature.example.test/northstar/work-004",
  },
  {
    recordId: "northstar-005",
    workId: "work-005",
    title: "Working-Time Reduction: A Meta-analysis",
    authors: ["Nora Patel", "Samir Holt"],
    year: 2021,
    doi: "10.5555/fdw.005",
    studyDesign: "Systematic review and meta-analysis",
    population: "27 studies covering manufacturing, services, and government",
    finding: "Wellbeing effects were usually positive; output effects varied by workload and staffing.",
    limitation: "Most included studies were non-randomized and used different output measures.",
    status: "published",
    sourceUrl: "https://literature.example.test/northstar/work-005",
  },
  {
    recordId: "northstar-006",
    workId: "work-006",
    title: "Productivity Effects of a Four-Day Week in Retail",
    authors: ["Martin Cole", "Yuki Ito"],
    year: 2020,
    doi: "10.5555/fdw.006",
    studyDesign: "Retrospective observational study",
    population: "2,400 retail workers in one national chain",
    finding: "The paper reported a 7% productivity gain after a schedule change.",
    limitation: "A data audit found duplicated stores and an incorrect denominator.",
    status: "retracted",
    retractionNotice: "Retracted in 2024 because the productivity analysis used duplicated store records.",
    sourceUrl: "https://literature.example.test/northstar/work-006",
  },
];

const indexB: Paper[] = [
  {
    recordId: "harbor-001",
    workId: "work-002",
    title: "Reduced Hours and Customer-Service Throughput",
    authors: ["Luis Ortega", "Priya Shah"],
    year: 2023,
    doi: "10.5555/fdw.002",
    studyDesign: "Prospective cohort study",
    population: "18 customer-service centers followed for nine months",
    finding: "Burnout fell, but average ticket throughput was 4% lower after hours were reduced.",
    limitation: "Centers selected their own schedules and used different staffing ratios.",
    status: "published",
    sourceUrl: "https://literature.example.test/harbor/work-002-record",
  },
  {
    recordId: "harbor-002",
    workId: "work-004",
    title: "Compressed Workweeks in Public Clinics",
    authors: ["Elena Rossi", "David Kim"],
    year: 2025,
    doi: "10.5555/fdw.004",
    studyDesign: "Cluster randomized trial",
    population: "40 outpatient clinics and 1,120 clinical staff",
    finding: "Staff wellbeing improved while appointment completion remained statistically unchanged.",
    limitation: "The nine-month trial did not measure long-term retention or patient outcomes.",
    status: "published",
    sourceUrl: "https://literature.example.test/harbor/work-004-record",
  },
  {
    recordId: "harbor-003",
    workId: "work-006",
    title: "Productivity Effects of a Four-Day Week in Retail",
    authors: ["Martin Cole", "Yuki Ito"],
    year: 2020,
    doi: "10.5555/fdw.006",
    studyDesign: "Retrospective observational study",
    population: "2,400 retail workers in one national chain",
    finding: "The paper reported a 7% productivity gain after a schedule change.",
    limitation: "A data audit found duplicated stores and an incorrect denominator.",
    status: "retracted",
    retractionNotice: "Retracted in 2024 because the productivity analysis used duplicated store records.",
    sourceUrl: "https://literature.example.test/harbor/work-006-record",
  },
  {
    recordId: "harbor-004",
    workId: "work-007",
    title: "A Staggered Four-Day Week and Hospital Output",
    authors: ["Fatima Noor", "Glen Wu"],
    year: 2024,
    doi: "10.5555/fdw.007",
    studyDesign: "Difference-in-differences study",
    population: "13 hospitals adopting staggered schedules and 13 controls",
    finding: "The adopting hospitals maintained discharge volume, with larger gains in units that added staff.",
    limitation: "Hospitals chose adoption dates and staffing changes were not uniform.",
    status: "published",
    sourceUrl: "https://literature.example.test/harbor/work-007",
  },
  {
    recordId: "harbor-005",
    workId: "work-008",
    title: "Employee Preferences for Shorter Workweeks",
    authors: ["Sofia Marin", "Owen Price"],
    year: 2023,
    doi: "10.5555/fdw.008",
    studyDesign: "Cross-sectional survey",
    population: "8,700 workers recruited from an online panel",
    finding: "Most respondents preferred a four-day schedule and expected better wellbeing.",
    limitation: "Preferences and expectations do not establish effects on measured output.",
    status: "published",
    sourceUrl: "https://literature.example.test/harbor/work-008",
  },
  {
    recordId: "harbor-006",
    workId: "work-009",
    title: "Four-Day Workweek and Service Quality: Trial Protocol",
    authors: ["Hana Brooks", "Ibrahim Saleh"],
    year: 2025,
    doi: "10.5555/fdw.009",
    studyDesign: "Registered trial protocol",
    population: "Planned sample of 60 municipal service teams",
    finding: "The protocol defines wellbeing and service-output measures but reports no results.",
    limitation: "It cannot provide evidence about effects until the trial is completed.",
    status: "published",
    sourceUrl: "https://literature.example.test/harbor/work-009",
  },
];

const docs = `# Synthetic literature indexes

Fixture version: ${fixtureVersion}

This deterministic service contains a synthetic literature profile and two
mock indexes. It has no connection to a publisher, scholarly index, author, or
real account. Every record is test data. The base URL is the URL that served
this document.

## Operations

- \`GET /v1/profile\`: return the review question, scope, and bounded search plan.
- \`GET /v1/index-a/search\`: return up to six records from Northstar Index.
- \`GET /v1/index-b/search\`: return up to six records from Harbor Index.

The search operations accept optional \`query\` and \`limit\` query parameters.
The fixture returns six records per index and a null \`next_page_token\`. Each
record includes a stable \`work_id\` and DOI. The same work can occur in both
indexes under different \`record_id\` and source URLs. Compare work ID, DOI,
title, authors, year, and content before counting a work twice.

Each record also supplies study design, population, finding, limitation,
status, and source URL. A retracted record includes a retraction notice. Keep
conflicting findings and distinguish results from protocols, surveys, and
commentary. The service is read-only and has no write operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

function paperResponse(paper: Paper) {
  return {
    record_id: paper.recordId,
    work_id: paper.workId,
    title: paper.title,
    authors: paper.authors,
    year: paper.year,
    doi: paper.doi,
    study_design: paper.studyDesign,
    population: paper.population,
    finding: paper.finding,
    limitation: paper.limitation,
    status: paper.status,
    ...(paper.retractionNotice ? { retraction_notice: paper.retractionNotice } : {}),
    source_url: paper.sourceUrl,
  };
}

function searchResponse(indexName: string, papers: Paper[]) {
  return {
    index: indexName,
    query: "four-day workweek wellbeing service output",
    searched_at: "2026-09-08",
    papers: papers.map(paperResponse),
    next_page_token: null,
  };
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "literature", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json(profile);
    }
    if (request.method === "GET" && url.pathname === "/v1/index-a/search") {
      return json(searchResponse("Northstar Index", indexA));
    }
    if (request.method === "GET" && url.pathname === "/v1/index-b/search") {
      return json(searchResponse("Harbor Index", indexB));
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic literature indexes listening on http://127.0.0.1:${server.port}`);
