# 100 Common and Difficult Digital Personal Assistant Tasks

Research date: 2026-08-18

Noema assessment date: 2026-08-26

Noema assessment baseline: commit `6cb847c3`, plus pre-existing development worktree changes

## Executive summary

This report identifies 100 recurring outcomes that people can delegate to a digital personal assistant.

The list favors long duration, personal context, reasoning, cross-tool coordination, and large information sets.

These are common task families within their relevant populations. They are not a statistical ranking of all adults.

The research found five repeated patterns:

1. The most useful tasks close loops over time. They do not end after one answer or one tool call.
2. Personalization mainly means constraints, relationships, priorities, permissions, and prior decisions. It means more than writing style.
3. Many ordinary tasks cross institutional boundaries. Email, portals, calendars, files, payments, and forms rarely share one state.
4. The difficult step is often reconciliation. The assistant must compare changing, incomplete, duplicated, or conflicting records.
5. Consequential tasks need evidence, review points, and change receipts. A plausible answer is not sufficient.

This report is a research artifact. It does not replace the existing 50-case personal-agent evaluation suite.

## Scope

A digital personal assistant task has a user outcome and uses digital information or services.

The assistant can research, organize, recommend, monitor, draft, or act within explicit authority.

The list excludes four task types:

- A single lookup with one clear source
- A purely physical chore
- A professional diagnosis, legal judgment, or regulated financial decision
- An irreversible external action without review or prior policy

Some tasks concern a specific population. Caregiving tasks are common for caregivers, but not for every adult.

## Research method

The research used four evidence families:

1. Official time-use and service-use data show which activity families recur.
2. Government and institutional guides show the real steps, records, deadlines, and decisions.
3. Large surveys and field studies show workload, friction, and affected populations.
4. Agent benchmarks show why multi-step web, desktop, planning, and memory tasks remain difficult.

The sources validate task families. The exact task wording, grouping, and challenge signatures are this report's synthesis.

The numbering supports reference. It does not rank frequency, value, or difficulty.

The selection process used these rules:

- Include a task family supported by at least one strong evidence family.
- Require at least three difficulty factors from the key below.
- Require reasoning plus long duration, personal context, cross-tool work, or large information sets.
- Prefer a clear outcome with a visible completion condition.
- Keep high-risk decisions with the user or a qualified professional.

The research used English-language sources. Public evidence is weighted toward the United States, Europe, and other OECD countries.

## Challenge key

| Code | Difficulty factor | Meaning |
| --- | --- | --- |
| L | Long duration | State persists across days, months, or years and needs follow-through |
| P | Personal context | The task depends on preferences, relationships, history, limits, or prior decisions |
| R | Reasoning | The task needs judgment, constraint handling, reconciliation, or uncertainty management |
| X | Cross-tool work | The task reads or changes several services, files, accounts, or communication channels |
| I | Information scale | The task collects, deduplicates, compares, or summarizes many records |
| S | High stakes | Errors can cause material cost, lost rights, harm, privacy loss, or damaged relationships |

Every listed task has at least three factors. The codes describe difficulty, not permission to act.

## Evidence snapshot

Official time-use data establish a broad base for these tasks.

- The [2024 American Time Use Survey](https://www.bls.gov/news.release/archives/atus_06262025.htm) measures household management, purchases, care, work, education, and communication.
- The [OECD Time Use Database](https://www.oecd.org/en/data/datasets/time-use-database.html) covers paid work, unpaid work, study, care, and personal time across 30 countries.
- The [UK Time Use Survey](https://www.ons.gov.uk/peoplepopulationandcommunity/personalandhouseholdfinances/incomeandwealth/bulletins/timeuseintheuk/march2023) includes shopping, household administration, unpaid care, and digital-device use.
- The [OECD Employment Outlook 2025](https://www.oecd.org/en/publications/oecd-employment-outlook-2025_194a947b-en/full-report/component-6.html) identifies cognitive domestic work. This work includes anticipating needs, comparing options, deciding, and monitoring.

Digital administration is also widespread.

- [Eurostat](https://ec.europa.eu/eurostat/web/products-eurostat-news/w/ddn-20250226-1) reports that 70% of people aged 16–74 in the EU used online public services during 2024.
- The [European Commission's 2026 benchmark](https://digital-strategy.ec.europa.eu/en/library/digital-decade-2026-egovernment-benchmark-2026) tests 96 services across nine life events.
- The [U.S. Government Accountability Office](https://www.gao.gov/products/gao-25-107239) links benefit loss to application time, eligibility research, forms, and supporting documents.

Work evidence shows heavy coordination and information costs.

- Microsoft's [2023 Work Trend Index](https://www.microsoft.com/en-us/worklab/work-trend-index/will-ai-fix-work) reports 57% of Microsoft 365 time in meetings, chat, and email.
- The same study reports that 62% of respondents struggle with information search. It reports that 68% lack enough uninterrupted focus time.
- Asana's [2023 global survey](https://investors.asana.com/news-releases/news-release-details/asana-anatomy-work-global-index-2023-smart-collaboration-and/) reports that knowledge workers spend 58% of their day on coordination work.

Care and family evidence shows sustained coordination.

- [Caregiving in the US 2025](https://www.caregivingintheus.org/report_categories/caregiving-in-the-us-report-2025-downloads/) estimates 63 million adult caregivers in the United States.
- The report covers personal care, medical tasks, financial management, and service coordination.
- The [2023 national education survey](https://nces.ed.gov/pubs2024/2024113.pdf) reports school messages, meetings, events, conferences, and homework support.

Agent research confirms that ordinary digital work can be technically difficult.

- [AssistantBench](https://aclanthology.org/2024.emnlp-main.505/) contains 214 realistic, time-consuming web tasks collected from people and domain experts.
- [GAIA](https://proceedings.iclr.cc/paper_files/paper/2024/hash/25ae35b5b1738d80f1f03a8713e405ec-Abstract-Conference.html) tests reasoning, browsing, multimodal inputs, and tool use.
- [OSWorld](https://papers.nips.cc/paper_files/paper/2024/hash/5d413e48f84dc61244b6be550f1cd8f5-Abstract-Datasets_and_Benchmarks_Track.html) tests 369 workflows across real desktop and web applications.
- [LongMemEval](https://arxiv.org/abs/2410.10813) tests extraction, multi-session reasoning, temporal reasoning, updates, and abstention.

Benchmark scores are historical snapshots. They support the difficulty model, not a claim about current model rankings.

### Evidence coverage map

The anchors below support task recurrence, process structure, or difficulty. They do not prove every synthesized description word.

| Tasks | Evidence anchors |
| --- | --- |
| 1–10 | [American Time Use Survey](https://www.bls.gov/news.release/archives/atus_06262025.htm), [OECD cognitive domestic work](https://www.oecd.org/en/publications/oecd-employment-outlook-2025_194a947b-en/full-report/component-6.html), [Microsoft work study](https://www.microsoft.com/en-us/worklab/work-trend-index/will-ai-fix-work), and [email commitment research](https://www.microsoft.com/en-us/research/blog/email-overload-using-machine-learning-to-manage-messages-commitments/) |
| 11–18 | [Calendar.help](https://www.microsoft.com/en-us/research/publication/calendar-help-designing-workflow-based-scheduling-agent-humans-loop/), [O*NET administrative-assistant duties](https://www.onetonline.org/link/summary/43-6011.00), and [weak-tie field experiments](https://doi.org/10.1126/science.abl4476) |
| 19–27 | [WorkArena](https://arxiv.org/abs/2403.07718), [Asana's work survey](https://investors.asana.com/news-releases/news-release-details/asana-anatomy-work-global-index-2023-smart-collaboration-and/), and [Microsoft's work study](https://www.microsoft.com/en-us/worklab/work-trend-index/will-ai-fix-work) |
| 28–30 | [CareerOneStop](https://www.careeronestop.org/JobSearch/job-search.aspx) and [BLS job-search methods](https://www.bls.gov/cps/data/aa2024/cpsaat34.htm) |
| 31–37 | [AssistantBench](https://aclanthology.org/2024.emnlp-main.505/), [GAIA](https://proceedings.iclr.cc/paper_files/paper/2024/hash/25ae35b5b1738d80f1f03a8713e405ec-Abstract-Conference.html), and the [systematic-review labor study](https://doi.org/10.1136/bmjopen-2016-012545) |
| 38–40 | [OECD adult-learning research](https://www.oecd.org/en/publications/trends-in-adult-learning_ec0624a6-en.html) and the [systematic-review labor study](https://doi.org/10.1136/bmjopen-2016-012545) |
| 41–43 | [Federal Reserve household research](https://www.federalreserve.gov/publications/2025-economic-well-being-of-us-households-in-2024-income-and-expenses.htm), [CFPB household research](https://www.consumerfinance.gov/data-research/research-reports/making-ends-meet-in-2024-insights-from-the-making-ends-meet-survey/), and [FTC subscription guidance](https://consumer.ftc.gov/articles/getting-and-out-free-trials-auto-renewals-and-negative-option-subscriptions) |
| 44–45 | [NAIC insurance guidance](https://content.naic.org/consumer/how-does-insurance-work) and [HealthCare.gov cost guidance](https://www.healthcare.gov/choose-a-plan/your-total-costs/) |
| 46–48 | [GAO administrative-burden research](https://www.gao.gov/products/gao-25-107239), [GAO retirement-account research](https://www.gao.gov/products/gao-24-103577), and [Federal Reserve household research](https://www.federalreserve.gov/publications/2025-economic-well-being-of-us-households-in-2024-income-and-expenses.htm) |
| 49–52 | [USAGov address guidance](https://www.usa.gov/change-address), [FTC dispute guidance](https://consumer.ftc.gov/articles/solving-problems-business-returns-refunds-and-other-resolutions), the [NIA affairs checklist](https://www.nia.nih.gov/health/advance-care-planning/getting-your-affairs-order-checklist-documents-prepare-future), and [USAGov death guidance](https://www.usa.gov/report-a-death) |
| 53–64 | [Caregiving in the US](https://www.caregivingintheus.org/report_categories/caregiving-in-the-us-report-2025-downloads/), [AHRQ care coordination](https://www.ahrq.gov/ncepcr/care/coordination/atlas/chapter6.html), and [ONC portal research](https://healthit.gov/data/data-briefs/individuals-access-and-use-patient-portals-and-smartphone-health-apps-2024/) |
| 65–74 | [American Time Use Survey](https://www.bls.gov/news.release/archives/atus_06262025.htm), [CPSC recall resources](https://www.cpsc.gov/About-CPSC/Consumer-Resources), [FTC dispute guidance](https://consumer.ftc.gov/articles/solving-problems-business-returns-refunds-and-other-resolutions), and [Ready.gov records guidance](https://www.ready.gov/sites/default/files/2020-03/ready_emergency-financial-first-aid-toolkit-checklists-and-forms.pdf) |
| 75–84 | [TravelPlanner](https://iclr.cc/virtual/2024/22166), [COMPASS](https://machinelearning.apple.com/research/multi-turn-benchmark), the [State Department checklist](https://travel.state.gov/en/international-travel/planning/checklist.html), and [USAGov address guidance](https://www.usa.gov/change-address) |
| 85–92 | [NCES family-involvement data](https://nces.ed.gov/pubs2024/2024113.pdf), [Federal Student Aid](https://studentaid.gov/articles/evaluating-financial-aid-offers/), and the [American Time Use Survey](https://www.bls.gov/news.release/archives/atus_06262025.htm) |
| 93–100 | [Pew privacy research](https://www.pewresearch.org/internet/2023/10/18/how-americans-view-data-privacy/), [CISA security guidance](https://www.cisa.gov/secure-our-world), [Library of Congress archiving guidance](https://www.digitalpreservation.gov/personalarchiving/), and [W3C accessibility guidance](https://www.w3.org/WAI/people-use-web/) |

## The 100 tasks

### 1. Daily coordination and commitments

Microsoft's work data supports the scale of communication and search.

Its [email field research](https://www.microsoft.com/en-us/research/blog/email-overload-using-machine-learning-to-manage-messages-commitments/) found deferral when replies needed more time, resources, or priority.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 1 | Build a daily operational brief from current commitments and changes | Reconcile events, deadlines, urgent messages, preparation needs, travel, and personal capacity without duplicate items | Daily | Email, chat, calendar, tasks, notes | L P R X I |
| 2 | Maintain one trusted register of promises made by or to the user | Resolve implicit commitments, owners, dates, later changes, completion evidence, and appropriate follow-up | Days–months | Email, chat, meeting notes, tasks | L P R X I |
| 3 | Capture actions, decisions, and dates from incoming information | Separate real obligations from discussion, merge duplicates, preserve sources, and route each item correctly | Continuous | Email, chat, files, tasks, calendar | L P R X I |
| 4 | Maintain a prioritized reply and action queue | Infer which messages need action, apply relationship context, account for urgency, and avoid premature replies | Hours–weeks | Email, messaging, contacts, tasks | L P R X I |
| 5 | Audit the near-term calendar for conflicts and hidden load | Include travel, preparation, recovery time, focus needs, flexible events, and personal working limits | Days–weeks | Calendar, maps, tasks, contacts | L P R X I |
| 6 | Create a realistic daily plan around fixed commitments | Estimate work, protect buffers, apply energy preferences, and show work that does not fit | Daily | Calendar, tasks, notes, health data | L P R X |
| 7 | Replan the day after a delay, cancellation, or urgent request | Preserve priorities while recalculating dependencies, travel, deadlines, and communication obligations | Minutes–days | Calendar, tasks, messaging, maps | L P R X |
| 8 | Produce a weekly preview and prior-week review | Connect outcomes, open loops, meetings, deadlines, capacity, and future preparation across several projects | Weekly | Calendar, email, tasks, notes | L P R X I |
| 9 | Maintain a tracker for deadlines, renewals, and recurring obligations | Detect dates in scattered records, distinguish firm dates, monitor completion, and escalate before loss | Months–years | Email, calendar, portals, files | L P R X I S |
| 10 | Review progress against personal goals and adjust the plan | Compare stated priorities with actual time, completed work, changing constraints, and new opportunities | Weeks–years | Calendar, tasks, journal, finance data | L P R X I |

### 2. Communication and relationships

Microsoft Research describes email as both communication and a record for tasks, schedules, and collaboration.

Its [Calendar.help study](https://www.microsoft.com/en-us/research/publication/calendar-help-designing-workflow-based-scheduling-agent-humans-loop/) processed thousands of delegated meeting requests.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 11 | Resolve conflicting requests and prepare an escalation plan | Compare authority, deadlines, dependencies, relationship risks, and user priorities before proposing a response or negotiated sequence | Hours–weeks | Email, chat, calendar, tasks, contacts | L P R X I S |
| 12 | Monitor an important conversation and escalate only when needed | Track expected responses, material changes, deadlines, silence, and the user's preferred intervention threshold | Hours–months | Email, chat, tasks, notifications | L P R X I S |
| 13 | Coordinate a multi-person meeting across calendars and time zones | Resolve identities, availability, working hours, travel, priorities, duration, rooms, and repeated negotiation | Days–weeks | Email, calendar, contacts, video tools | L P R X I |
| 14 | Prepare a meeting brief and decision-focused agenda | Reconstruct history, attendee roles, prior decisions, open questions, desired outcomes, and sensitive boundaries | Hours–weeks | Calendar, email, notes, files, contacts | P R X I |
| 15 | Prepare audience-specific updates from one evidence set | Preserve facts while changing detail, tone, confidentiality, asks, and format for each audience | Hours–weeks | Email, chat, documents, project tools | P R X I S |
| 16 | Record meeting decisions and drive each follow-up to closure | Separate decisions from discussion, assign explicit owners, preserve evidence, monitor changes, and chase overdue actions | Days–months | Meeting records, tasks, email, calendar | L P R X I |
| 17 | Build and maintain a concise relationship brief | Resolve identities, summarize relevant history, track commitments, respect private boundaries, and link upcoming contact | Months–years | Contacts, email, calendar, notes | L P R X I S |
| 18 | Maintain a respectful relationship follow-up plan | Balance cadence, reciprocity, important dates, unresolved promises, and the risk of intrusive or mechanical contact | Months–years | Contacts, calendar, messaging, notes | L P R X |

### 3. Work, projects, and career

The [WorkArena benchmark](https://arxiv.org/abs/2403.07718) models common knowledge work inside enterprise software.

[CareerOneStop](https://www.careeronestop.org/JobSearch/job-search.aspx) treats job search as a campaign spanning research, networking, applications, interviews, and negotiation.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 19 | Produce an evidence-based project status from scattered activity | Reconcile plans, task state, messages, meetings, documents, decisions, blockers, and missing updates | Days–months | Project tools, chat, email, files, calendar | L P R X I |
| 20 | Detect project risks, dependencies, and commitments at risk | Infer relationships, distinguish weak signals from confirmed issues, estimate impact, and recommend the smallest intervention | Days–months | Tasks, plans, messages, calendars, documents | L P R X I S |
| 21 | Maintain a decision log with rationale and replacement history | Find confirmed decisions, preserve context and owners, link evidence, and mark later replacement without rewriting history | Months–years | Email, meeting notes, documents, project tools | L P R X I |
| 22 | Turn an ambiguous goal into an executable project plan | Clarify success, expose assumptions, identify dependencies, estimate effort, sequence milestones, and define review points | Weeks–years | Notes, tasks, calendar, files, research tools | L P R X I |
| 23 | Assemble a deliverable from scattered source material | Find authoritative inputs, resolve version conflicts, fill traceable gaps, and produce the required format | Hours–months | Email, drives, documents, spreadsheets, web | P R X I S |
| 24 | Coordinate a multi-reviewer approval process | Route correct versions, collect comments, resolve conflicts, track decisions, and prevent stale approval | Days–months | Documents, email, chat, project tools | L P R X I S |
| 25 | Build and submit an employer expense packet | Match receipts to transactions and workplace policy, allocate categories, explain exceptions, and track approval or payment | Days–months | Email, files, expense system, forms | L P R X I S |
| 26 | Prepare an onboarding, offboarding, or handoff package | Combine current state, responsibilities, access needs, decisions, contacts, risks, and unfinished work | Days–months | Identity tools, files, project tools, email | L P R X I S |
| 27 | Maintain professional credentials and compliance obligations | Track changing rules, required evidence, education credits, fees, approvals, and renewal dates | Months–years | Portals, email, calendar, files, payments | L P R X I S |
| 28 | Run a job-search pipeline from target research through follow-up | Match goals to roles, track many applications, coordinate contacts, and adapt from results over time | Weeks–months | Job sites, email, contacts, calendar, spreadsheet | L P R X I |
| 29 | Prepare truthful, tailored application packets | Map verified experience to each role, preserve accuracy, manage versions, and meet different submission requirements | Days–months | Job sites, documents, email, portfolio | L P R X I S |
| 30 | Compare job offers and prepare a negotiation decision packet | Normalize pay, benefits, risk, commute, growth, flexibility, preferences, and unanswered questions | Days–weeks | Documents, spreadsheets, email, benefits portals | P R X I S |

### 4. Research, learning, and personal knowledge

[AssistantBench](https://aclanthology.org/2024.emnlp-main.505/) uses realistic tasks that require broad web navigation and multi-source answers.

[GAIA](https://proceedings.iclr.cc/paper_files/paper/2024/hash/25ae35b5b1738d80f1f03a8713e405ec-Abstract-Conference.html) shows that simple human questions can require robust reasoning, browsing, and tool use.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 31 | Compare a major product or service under personal constraints | Search a changing market, normalize features and total cost, test compatibility, and weigh soft preferences | Days–months | Web, reviews, spreadsheets, notes, email | L P R X I S |
| 32 | Produce a current, source-grounded brief on an unfamiliar topic | Find authoritative and diverse evidence, resolve disagreement, separate facts from inference, and state coverage limits | Hours–weeks | Web, papers, files, notes | R X I S |
| 33 | Monitor a topic and report only material changes | Maintain a baseline, detect real changes, reject repetition, assess relevance, and update prior conclusions | Weeks–years | Web, alerts, email, database, notes | L P R X I |
| 34 | Build a literature review and evidence map | Search several indexes, deduplicate works, assess methods, connect claims, and expose gaps or contradictory findings | Days–months | Paper indexes, web, citation tools, documents | L P R X I |
| 35 | Fact-check a claim set and explain unresolved uncertainty | Trace claims to primary evidence, verify dates and definitions, compare sources, and avoid false certainty | Hours–weeks | Web, archives, files, spreadsheets | R X I S |
| 36 | Extract a structured inventory from many documents and sites | Handle mixed formats, identify entities, normalize fields, merge duplicates, preserve source history, and flag missing values | Hours–months | PDFs, images, email, web, spreadsheet | L R X I |
| 37 | Analyze a personal dataset and produce a decision-ready explanation | Clean data, select valid comparisons, test anomalies, visualize relevant patterns, and connect results to the user's question | Hours–weeks | Spreadsheets, databases, notebooks, documents | P R X I S |
| 38 | Maintain a useful reading and newsletter digest | Learn topic priorities, remove repeated coverage, preserve valuable links, and adapt depth to available attention | Weeks–years | Email, feeds, web, notes, reading apps | L P R X I |
| 39 | Maintain a personalized learning plan that adapts to progress | Map goals and prior knowledge, schedule practice, detect gaps, revise pacing, and connect learning to real work | Months–years | Courses, calendar, notes, quizzes, tasks | L P R X I |
| 40 | Compare courses, programs, or credentials and manage the choice | Reconcile prerequisites, outcomes, quality, cost, aid, schedules, deadlines, and personal goals | Weeks–years | Education sites, portals, email, spreadsheet | L P R X I S |

### 5. Money, tax, insurance, benefits, and personal administration

The [Federal Reserve's 2024 household survey](https://www.federalreserve.gov/publications/2025-economic-well-being-of-us-households-in-2024-income-and-expenses.htm) reports bill pressure, variable income, savings, credit, and financial shocks.

The [January 2024 CFPB survey](https://www.consumerfinance.gov/data-research/research-reports/making-ends-meet-in-2024-insights-from-the-making-ends-meet-survey/) found that 42.9% of households had difficulty paying an expense or bill during the previous year.

The [IRS 2025 instructions](https://www.irs.gov/pub/irs-prior/i1040gi--2025.pdf) estimate 12 hours for an average individual return. Business filers have higher estimated burdens.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 41 | Maintain a household cash-flow and bill plan | Reconcile variable income, due dates, autopay, shared accounts, priorities, penalties, and future shortfalls | Weeks–years | Banks, billers, email, calendar, spreadsheet | L P R X I S |
| 42 | Build and maintain the annual tax-readiness packet | Collect records throughout the year, classify them correctly, detect missing forms, track estimates, and support professional review | Months–years | Financial accounts, email, files, tax tools | L P R X I S |
| 43 | Audit subscriptions and complete cancellations or renewals | Find charges across accounts, reconstruct terms, compare value, meet notice periods, retain proof, and verify billing stops | Months–years | Banks, email, merchant portals, calendar | L P R X I |
| 44 | Review insurance coverage against current household risks | Normalize limits, exclusions, deductibles, networks, assets, life changes, and total costs across several policies | Months–years | Policy portals, files, quotes, spreadsheet | L P R X I S |
| 45 | Reconcile a property, vehicle, disability, or life-insurance claim | Match loss evidence, policy terms, estimates, receipts, deadlines, correspondence, payments, and appeal evidence | Weeks–years | Insurer portals, repair estimates, files, email | L P R X I S |
| 46 | Find eligible benefits and manage applications or renewals | Compare program rules, household definitions, evidence, agencies, interviews, reporting duties, and recertification dates | Weeks–years | Benefit finders, portals, forms, files, calendar | L P R X I S |
| 47 | Consolidate retirement records and prepare rollover decisions | Find old accounts, compare fees and rules, track transfers, verify completion, and preserve tax records | Months–years | Employer portals, plan providers, files, mail | L P R X I S |
| 48 | Maintain a credit and debt record and resolve errors | Reconcile bureaus, lenders, balances, rates, payment plans, disputes, evidence, and repeated status checks | Months–years | Credit reports, lender portals, banks, files | L P R X I S |
| 49 | Maintain official documents, licenses, and identity renewals | Track different validity rules, photos, forms, fees, appointments, processing times, travel needs, and household members | Months–years | Government portals, calendar, files, payments | L P R X I S |
| 50 | Escalate an unresolved consumer dispute through formal channels | Assemble prior attempts, contracts, receipts, payment evidence, requested remedy, deadlines, complaint routes, and response status | Days–months | Merchant portals, email, banks, complaint sites | L P R X I S |
| 51 | Maintain an important-affairs and estate document map | Discover sensitive preferences, assets, beneficiaries, trusted people, legal documents, secure access, and life-event updates | Months–years | Files, financial portals, contacts, legal forms | L P R X I S |
| 52 | Administer a deceased person's accounts, benefits, and required notices | Coordinate certificates, benefits, taxes, banks, insurers, utilities, subscriptions, property, digital accounts, deadlines, and authorized access | Weeks–years | Government portals, banks, insurers, files, email | L P R X I S |

The assistant should not make regulated decisions in this section.

The user or a qualified professional should approve filings, appeals, contracts, transfers, and investment choices.

### 6. Health and caregiving

[Caregiving in the US 2025](https://www.caregivingintheus.org/report_categories/caregiving-in-the-us-report-2025-downloads/) reports that caregivers handle medical tasks, personal care, money, and service coordination.

The [AHRQ Care Coordination Atlas](https://www.ahrq.gov/ncepcr/care/coordination/atlas/chapter6.html) covers accountability, information transfer, transitions, needs, goals, and proactive care plans.

[ONC's 2024 data brief](https://healthit.gov/data/data-briefs/individuals-access-and-use-patient-portals-and-smartphone-health-apps-2024/) reports that proxy access to another person's portal rose from 24% to 51% between 2020 and 2024.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 53 | Consolidate a complete, current medical record | Collect records from several providers, normalize names and dates, reconcile conflicts, protect access, and show missing items | Months–years | Patient portals, insurer, pharmacy, files | L P R X I S |
| 54 | Maintain a verified medication and refill plan | Reconcile prescriptions, actual use, doses, prescribers, pharmacies, changes, refill timing, and questions for clinicians | Weeks–years | Pharmacy, portals, calendar, secure notes | L P R X I S |
| 55 | Prepare a focused medical appointment brief | Organize symptoms, measurements, prior advice, tests, medications, goals, and the most important unanswered questions | Days–months | Health diary, portals, files, calendar | L P R X I S |
| 56 | Coordinate referrals, tests, specialists, and transport | Track orders, network status, prerequisites, records transfer, availability, travel, preparation, results, and follow-up | Weeks–months | Portals, insurer, calendar, maps, phone | L P R X I S |
| 57 | Monitor a care plan and close every follow-up loop | Compare instructions with appointments, measurements, laboratory results, refills, warning thresholds, and later plan changes | Weeks–years | Portals, devices, calendar, tasks, messaging | L P R X I S |
| 58 | Compare suitable providers or care services | Apply network, specialty, access, location, language, accessibility, quality, cost, relationship, and availability needs | Days–months | Directories, insurer, maps, reviews, phone | P R X I S |
| 59 | Compare health plans against expected household care | Model premiums, cost sharing, networks, drugs, likely care, tax effects, and worst-case exposure | Days–months | Plan marketplace, formularies, files, spreadsheet | L P R X I S |
| 60 | Build a health prior-authorization, claim, or appeal packet | Trace requirements, denials, clinical records, policy language, deadlines, submissions, calls, and decision status | Weeks–years | Health insurer, provider portals, forms, files, mail | L P R X I S |
| 61 | Coordinate a safe discharge or care transition | Connect instructions, medicines, equipment, home support, transport, appointments, costs, warning signs, and caregiver training | Days–months | Hospital portal, pharmacy, insurer, calendar | L P R X I S |
| 62 | Maintain a shared caregiver schedule and responsibility plan | Balance availability, skills, consent, respite, appointments, finances, handoffs, and changing care needs | Weeks–years | Shared calendar, tasks, portals, messaging | L P R X I S |
| 63 | Summarize care-recipient changes for authorized people | Compare observations over time, separate facts from interpretation, preserve privacy, and route urgent concerns correctly | Days–years | Health logs, messaging, portals, files | L P R X I S |
| 64 | Prepare a treatment or second-opinion decision brief | Compare evidence, benefits, harms, practical burdens, user values, uncertainty, and questions for qualified clinicians | Days–months | Medical records, research, notes, portals | P R X I S |

The assistant should not diagnose, prescribe, or choose treatment.

It should preserve source evidence and route urgent symptoms to approved medical channels.

### 7. Home, household, and consumer operations

The [American Time Use Survey](https://www.bls.gov/news.release/archives/atus_06262025.htm) records household management, purchases, repairs, pet care, and service coordination.

The OECD also identifies hidden cognitive work. This work includes anticipation, searching options, decisions, and monitoring.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 65 | Maintain a household asset, receipt, warranty, and recall inventory | Identify products from mixed records, link serials and owners, monitor recalls, and preserve repair or replacement evidence | Months–years | Email, photos, files, retailer sites, recall data | L P R X I S |
| 66 | Run a preventive home-maintenance program | Learn each asset, climate, season, history, warranty, cost, access rule, and preferred service interval | Months–years | Manuals, calendar, vendors, tasks, receipts | L P R X I S |
| 67 | Coordinate a home repair or improvement project | Define scope, compare bids, verify credentials, sequence access, manage changes, document quality, and track payment | Weeks–years | Vendor sites, email, files, calendar, payments | L P R X I S |
| 68 | Optimize utilities and communication services | Normalize usage, rates, equipment, contracts, promotions, reliability, moving dates, and cancellation costs | Months–years | Provider portals, bills, email, spreadsheet | L P R X I |
| 69 | Maintain a meal and grocery plan for the household | Balance diet, allergies, preferences, budget, inventory, schedules, leftovers, stores, and changing attendance | Days–months | Grocery sites, calendar, recipes, inventory | L P R X I S |
| 70 | Maintain vehicle records and coordinate the vehicle lifecycle | Track maintenance, recalls, registration, insurance, repairs, costs, inspections, and eventual replacement | Months–years | Vehicle apps, government portals, shops, files | L P R X I S |
| 71 | Coordinate recurring pet care | Track health records, medicines, food, appointments, licenses, insurance, travel care, and emergency contacts | Months–years | Vet portals, pharmacy, calendar, files | L P R X I S |
| 72 | Complete a routine return, warranty, or repair request | Match purchases to standard policies, provide required evidence, meet deadlines, handle shipping, and verify the stated remedy | Days–months | Email, retailer portals, files, shipping | L P R X I |
| 73 | Source and manage recurring household services | Learn access, quality, schedule, budget, privacy, pet, cancellation, and backup-provider requirements | Weeks–years | Vendor sites, calendar, messaging, payments | L P R X I S |
| 74 | Maintain a household emergency-readiness plan | Keep contacts, documents, supplies, alerts, evacuation routes, accessibility needs, pets, backups, and review dates current | Months–years | Files, alerts, maps, contacts, inventory | L P R X I S |

The [CPSC recall service](https://www.cpsc.gov/About-CPSC/Consumer-Resources), [FTC resolution guide](https://consumer.ftc.gov/articles/solving-problems-business-returns-refunds-and-other-resolutions), and [Ready.gov toolkit](https://www.ready.gov/sites/default/files/2020-03/ready_emergency-financial-first-aid-toolkit-checklists-and-forms.pdf) show these workflows.

### 8. Travel, moves, housing, and events

[TravelPlanner](https://iclr.cc/virtual/2024/22166) models travel as long-duration planning over millions of records and interdependent constraints.

Apple's [COMPASS research](https://machinelearning.apple.com/research/multi-turn-benchmark) adds changing preferences and coordination across flights, lodging, and tickets.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 75 | Build one live itinerary from all confirmations and messages | Match bookings to travelers, local times, locations, reference numbers, cancellation terms, transfers, and missing segments | Days–months | Email, calendar, booking sites, maps, notes | L P R X I S |
| 76 | Plan and book a multi-leg trip under hard and soft constraints | Coordinate budget, timing, routes, lodging, activities, meals, fatigue, preferences, and cancellation risk | Days–months | Search, booking sites, maps, calendar, payments | L P R X I S |
| 77 | Monitor disruptions and prepare safe rebooking choices | Evaluate live changes, passenger rights, downstream effects, alternative routes, costs, accessibility, and user approval | Hours–weeks | Carrier apps, alerts, maps, booking sites | L P R X I S |
| 78 | Maintain an international entry and readiness checklist | Resolve nationality, transit, visas, passport validity, medicines, vaccines, insurance, custody papers, and changing rules | Weeks–years | Government sites, portals, files, calendar | L P R X I S |
| 79 | Track travel credits, disruption refunds, and travel-insurance claims | Match travel terms to disruptions and purchases, meet deadlines, retain evidence, and avoid losing restricted value | Weeks–years | Loyalty sites, travel insurers, email, files, calendar | L P R X I S |
| 80 | Coordinate group, family, or accessible travel | Gather several people's needs, resolve conflicts fairly, verify accessibility, and keep one feasible shared plan | Days–months | Surveys, messaging, booking sites, shared calendar | L P R X I S |
| 81 | Plan a personal event from guest list through closeout | Balance purpose, budget, venue, vendors, availability, access, communications, payments, changes, and follow-up | Weeks–years | Contacts, invitations, calendar, vendors, payments | L P R X I S |
| 82 | Run a move and propagate every required change | Sequence housing, movers, utilities, mail, government records, banks, insurance, schools, services, and confirmations | Weeks–months | Portals, email, calendar, files, payments | L P R X I S |
| 83 | Maintain a housing search against real personal constraints | Compare total cost, commute, access, schools, pets, safety, lease terms, availability, and application status | Weeks–months | Listing sites, maps, files, spreadsheet, messaging | L P R X I S |
| 84 | Complete a post-trip or post-move closeout | Reconcile expenses, refunds, claims, deposits, records, photos, open tasks, and useful lessons | Days–months | Banks, email, files, notes, portals | L P R X I S |

The [State Department checklist](https://travel.state.gov/en/international-travel/planning/checklist.html) and [DOT disruption dashboard](https://www.transportation.gov/airconsumer/airline-cancellation-delay-dashboard) show real travel dependencies.

[USAGov's address guide](https://www.usa.gov/change-address) shows that one move can require changes across several government services.

### 9. Family, education, relationships, and community

The [2023 national education survey](https://nces.ed.gov/pubs2024/2024113.pdf) reports frequent school messages, events, meetings, conferences, and homework support.

[Federal Student Aid](https://studentaid.gov/articles/evaluating-financial-aid-offers/) shows that education choices require cost, aid, debt, deadline, and preference comparisons.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 85 | Maintain one feasible family schedule and transport plan | Reconcile several calendars, custody, work, school, care, travel time, vehicles, backups, and permissions | Days–years | Calendars, school apps, maps, messaging | L P R X I S |
| 86 | Turn school communications into one actionable family digest | Merge newsletters, teacher messages, portals, forms, deadlines, schedule changes, payments, and child-specific context | Days–years | Email, school portals, calendar, files | L P R X I S |
| 87 | Coordinate childcare, camps, and child activities | Compare fit, cost, schedules, waitlists, transport, forms, health needs, equipment, payments, and changes | Weeks–years | Provider sites, portals, calendar, files, payments | L P R X I S |
| 88 | Make household and care responsibilities visible and balanced | Detect invisible planning work, respect skills and capacity, negotiate ownership, and monitor fairness without surveillance | Weeks–years | Shared tasks, calendar, messaging, notes | L P R X I S |
| 89 | Run a college, training, or scholarship application campaign | Track choices, prerequisites, essays, records, recommendations, aid, contributor steps, corrections, and deadlines | Months–years | School sites, aid portals, email, files | L P R X I S |
| 90 | Maintain authorized family records and permission packets | Keep health, education, custody, identity, emergency, consent, and travel records current and correctly shared | Months–years | Portals, files, contacts, secure sharing | L P R X I S |
| 91 | Match and coordinate community, civic, or volunteer participation | Apply interests, skills, access, location, schedule, eligibility, forms, training, commitments, and renewal dates | Weeks–years | Community sites, government portals, calendar, email | L P R X I |
| 92 | Maintain an annual relationship-care and occasion plan | Remember people, preferences, cultural practices, budgets, travel, lead times, shared duties, and changing relationships | Months–years | Contacts, calendar, notes, shopping, messaging | L P R X I |

### 10. Digital life, privacy, records, and resilience

Pew's [2023 privacy survey](https://www.pewresearch.org/internet/2023/10/18/how-americans-view-data-privacy/) found that 69% felt overwhelmed by their password count.

The same survey found that 34% experienced at least one listed fraud or account-security problem during the prior year.

| # | Task | Why it is difficult | Typical horizon | Typical systems | Signature |
| ---: | --- | --- | --- | --- | --- |
| 93 | Organize and preserve personal files, email, and photos | Find distributed collections, deduplicate safely, retain meaning, choose durable formats, protect privacy, and verify backups | Months–years | Devices, cloud drives, email, photo services | L P R X I S |
| 94 | Maintain an account and security posture inventory | Discover accounts, rank sensitivity, improve authentication, update recovery methods, remove stale access, and verify changes | Months–years | Password manager, email, account portals, devices | L P R X I S |
| 95 | Coordinate compromised-account or identity-theft recovery | Contain damage, preserve evidence, reset trust, contact institutions, dispute fraud, monitor reports, and track every recovery step | Hours–years | Email, banks, bureaus, government sites, files | L P R X I S |
| 96 | Review privacy settings and reduce unwanted data exposure | Inventory services, interpret permissions, apply user preferences, revoke access, request deletion, and verify later reappearance | Weeks–years | Account portals, devices, email, privacy services | L P R X I S |
| 97 | Complete a device replacement, migration, and backup verification | Preserve accounts, files, messages, keys, settings, authenticator access, application state, and secure disposal | Hours–weeks | Devices, cloud services, password manager, backups | L P R X I S |
| 98 | Maintain a personal knowledge base and decision history | Capture useful evidence, connect entities, update changed facts, preserve chronology, remove duplication, and support later retrieval | Months–years | Notes, files, email, calendar, search | L P R X I |
| 99 | Prepare and maintain a digital-legacy plan | Inventory accounts and media, record intentions, assign trusted roles, respect platform rules, and update after life changes | Months–years | Password manager, files, estate records, contacts | L P R X I S |
| 100 | Make personal digital information usable with assistive technology | Convert inaccessible content, preserve meaning, learn user needs, validate output, and retain an accessible source copy | Days–years | Documents, portals, files, assistive technology | L P R X I S |

The [CISA Secure Our World guide](https://www.cisa.gov/secure-our-world) covers passwords, multifactor authentication, phishing, and updates.

The [FTC recovery guide](https://consumer.ftc.gov/articles/what-know-about-identity-theft) spans credit bureaus, businesses, debt collectors, evidence, forms, and progress tracking.

The [Library of Congress](https://www.digitalpreservation.gov/personalarchiving/) covers personal records, email, photos, audio, video, websites, formats, descriptions, and storage.

The [W3C accessibility guide](https://www.w3.org/WAI/people-use-web/) explains how inaccessible digital tools create barriers across many abilities.

## Noema capability assessment

This section assesses Noema at the stated baseline. It is an implementation snapshot, not a roadmap or delivery promise.

The assessment uses these current authorities:

- The [Tasks contract](tasks.md) defines durable working directories, recurring work, review, gates, and deferred collaboration features.
- The [Memory contract](memory.md) defines evidence-backed local-human memory and its missing history, editing, private scopes, and additional scopes.
- The [Capability contract](harness/capabilities.md) defines governed MCP and native HTTP integrations.
- The [browser contract](harness/web-browsing.md) defines interactive browsing, public downloads, document parsing, and remaining browser limits.
- The [50-case ledger](validation/personal-agent-50-case-ledger.md) supplies dated live evidence for Gmail, Google Calendar, and Notion workflows.
- The [14-case live ledger](validation/difficult-personal-assistant-test-ledger-2026-08-26.md) records exact acceptance evidence for every row previously marked `Test`.
- The [100-task roadmap](plans/2026-08-27-personal-assistant-100-task-roadmap.md) sequences shared fixes, new systems, domain work, and live exit gates.
- The [proactive-event plan](plans/2026-08-15-proactive-event-sources.md) states that implementation has not started.
- The [current context](context/current.md) records implemented file tools and remaining live Gmail and Calendar OAuth acceptance.

Named third-party services identify the tested user setup. They do not define Noema core requirements.

Roadmap work targets provider-neutral behaviors. Concrete adapters retain provider authentication, mapping, pagination, and recovery.

Primary chats and Tasks now have durable working directories.

`file.download` stores public non-HTML resources. `file.parse` converts supported documents into bounded text.

Supported documents include PDF, Office, OpenDocument, EPUB, RTF, Excel, CSV, XML, JSON, Markdown, and plain text.

Noema still lacks user file uploads, browser uploads, image OCR, general export formats, and deterministic calculation tools.

Every row below was reassessed against the current contracts. Each row names the smallest reusable improvement that closes its demonstrated gap.

| Status | Meaning |
| --- | --- |
| Verified | One current live acceptance case passed. This result is evidence, not a permanent guarantee. |
| Test | A close implementation path exists. Noema needs an exact acceptance case and current connection proof. |
| Extend | Core primitives exist. Noema needs bounded connectors, data, rules, or reliability work. |
| Build | A central data, authority, integration, or execution system is absent. |

The first live suite produced five passes, eight partial results, and one failure.

The 2026-08-28 retests promoted Tasks 15 and 19 after both native Project cases passed.

Current retests verify 31 tasks. No task remains in `Test`.

Since 2026-08-18, Tasks 36 and 100 moved from Build to Extend.

Tasks 1, 2, 3, 5, 6, 7, 8, 9, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 24, 26, 28, 31, 32, 33, 35, 38, 39, 75, and 98 are now Verified.

These results prove one provider setup. The second provider portability gate remains open.

### 1. Daily coordination and commitments

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 1 | Build a daily operational brief | Verified | A delegated live case used non-empty messages, Calendar results, Tasks, and Memory. It preserved bounds, identifiers, deductions, and source limits. |
| 2 | Maintain one trusted promise register | Verified | A provider-neutral Repeat case carried one source-linked promise through an unrelated change. It closed only after an exact completion receipt. Test longer and multi-promise series before adding dedicated state. |
| 3 | Capture actions, decisions, and dates | Verified | A provider-neutral Project case created one native Inbox Task and one scheduled Task. It preserved exact source fields and deduplicated a repeat run. Connected source intake remains separate. |
| 4 | Maintain a prioritized reply and action queue | Extend | Add thread identity, relationship priority, mail drafts, reply operations, and sent-reply reconciliation. Revalidate the current Gmail connection. |
| 5 | Audit the calendar for conflicts and hidden load | Verified | Bounded live audits reached natural ends. They preserved recurrence parents, all-day dates, attendees, locations, overlaps, gaps, and cited route time. |
| 6 | Create a realistic daily plan | Verified | A delegated live case used a fixed cutoff, full Task reads, event evidence, preferences, breaks, and overload handling. It passed without correction. |
| 7 | Replan after disruption | Verified | A provider-neutral Repeat case moved one delayed event and its dependent chain around fixed work. It preserved unaffected work and drafted both required notices. Connected updates remain separate. |
| 8 | Produce a weekly preview and review | Verified | A delegated live case covered seven Repeat occurrences and full Task documents. It joined work, events, messages, Memory, deadlines, and preparation. |
| 9 | Track deadlines, renewals, and recurring obligations | Verified | A provider-neutral Repeat merged duplicate source mentions, excluded a soft target, emitted one overdue draft, stayed quiet twice, and closed only after timely exact evidence. Connected intake remains separate. |
| 10 | Review personal goals and adjust the plan | Build | Add goal outcomes, measures, target dates, review history, and links to Tasks and actual time. Projects do not supply this authority. |

### 2. Communication and relationships

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 11 | Resolve conflicting requests | Verified | The synthetic live case passed authority ranking, relationship trade-offs, escalation drafts, and safe non-action. Add a source-backed cross-system conflict case later. |
| 12 | Monitor an important conversation | Verified | A provider-neutral Repeat case stayed quiet before an agreed deadline and reported silence after it. It preserved exact message evidence and drafted one unsent reminder. Connected event intake remains separate. |
| 13 | Coordinate a multi-person meeting | Verified | A provider-neutral Project case handled three time zones, working-hours boundaries, one decline, five options, and draft-only replies and invitation details. Connected availability and invitation operations remain separate. |
| 14 | Prepare a meeting brief and agenda | Verified | A current live case found a qualifying external meeting and produced a cited, sensitive brief. Parsed attachments and conflicting prior decisions still need a fixture. |
| 15 | Prepare audience-specific updates | Verified | A native Project case produced three fact-matched updates. Full Task reads supported detail, tone, confidentiality, asks, citations, and conflict reporting. |
| 16 | Record decisions and close follow-ups | Verified | A provider-neutral Repeat preserved a superseded decision, tracked three owned follow-ups, closed only on exact timely receipts, stopped reminders, and stayed quiet when unchanged. Connected transcript intake remains separate. |
| 17 | Maintain a relationship brief | Verified | One exact email safely linked Memory, messages, and events. The reviewed brief preserved source bounds, staleness, commitments, and private boundaries. |
| 18 | Maintain a relationship follow-up plan | Verified | A provider-neutral Repeat preserved exact identity, reciprocity, cadence, consent, unanswered limits, and one user-owned promise. It closed on exact evidence and stayed quiet when unchanged. Connected interaction intake remains separate. |

### 3. Work, projects, and career

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 19 | Produce an evidence-based project status | Verified | A native Project case joined the Project and four full Task documents. It reconciled workstream reports with native stages and conflicts. |
| 20 | Detect project risks and dependencies | Verified | A provider-neutral Repeat reconstructed an exact six-Task dependency graph. It detected blockers, owner-specific capacity, a changed critical path, and one resolved risk. It proposed one bounded intervention and stayed quiet when unchanged. |
| 21 | Maintain a decision log | Verified | A provider-neutral Project case retained exact source history, marked one replaced decision superseded, and linked its replacement both ways. Test normally ordered longer series before adding a dedicated decision entity. |
| 22 | Turn an ambiguous goal into a project plan | Verified | A provider-neutral Project case converted an ambiguous goal into six native Tasks. Their documents preserved owners, estimates, targets, exact predecessor IDs, acceptance evidence, authority, and stop conditions. A reconciliation run reused all six. |
| 23 | Assemble a deliverable from scattered material | Extend | Public downloads and PDF or Office parsing now work. Add user file intake, version comparison, and required-format export. |
| 24 | Coordinate a multi-reviewer approval | Verified | A provider-neutral Repeat enforced roster membership, exact version binding, quorum, mandatory review, conflict resolution, and quiet unchanged delivery. Connected routing and response intake remain separate. |
| 25 | Build an employer expense packet | Build | Document parsing helps with statements. Add receipt uploads, image OCR, transaction matching, policy checks, expense writes, and reimbursement reconciliation. |
| 26 | Prepare onboarding, offboarding, or handoff | Verified | A provider-neutral Repeat built an audience-bound package, protected private and secret material, enforced grant-before-revoke ordering, preserved accountable ownership, closed on exact receipts, and stayed quiet. Connected lifecycle operations remain separate. |
| 27 | Maintain credentials and compliance obligations | Extend | Public evidence parsing now works. Add compliance records, private file intake, rule monitoring, portal uploads, fees, and renewal receipts. |
| 28 | Run a job-search pipeline | Verified | A provider-neutral Repeat deduplicated exact listings, required application receipts, tracked outcomes and contacts, adapted from exact feedback, drafted one safe next action, and stayed quiet when unchanged. Connected job, message, and application intake remains separate. |
| 29 | Prepare tailored application packets | Extend | Noema can parse source documents. Add user file intake, verified career records, templates, document generation, version control, and portal uploads. |
| 30 | Compare job offers and prepare negotiation | Extend | Noema can parse downloaded benefit documents. Add private file intake, compensation normalization, location data, and deterministic calculations. |

### 4. Research, learning, and personal knowledge

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 31 | Compare a major product or service | Verified | The live case passed current pricing, compatibility, warranty, source disagreement, budget handling, and preference weighting. Add tax and shipping in a future regression. |
| 32 | Produce a current research brief | Verified | The live case passed primary-source research, claim citations, uncertainty, inference separation, and recommendations. Retain citation regressions across provider changes. |
| 33 | Monitor a topic for material changes | Verified | A provider-neutral Repeat case kept a baseline through prior Task reads. It suppressed an unchanged update and reported one exact change after restart. Test a longer series before adding checkpoint storage. |
| 34 | Build a literature review and evidence map | Extend | PDF parsing now works. Add scholarly-index adapters, citation management, DOI deduplication, reproducible screening, and evidence-map structures. |
| 35 | Fact-check claims and uncertainty | Verified | The live case passed primary-source tracing, exact dates, definitions, scope limits, and explicit uncertainty. Archived historical evidence still needs a later case. |
| 36 | Extract a structured inventory | Extend | Public PDF, Office, and Excel parsing now works. Add user and batch intake, image OCR, structured tables, deduplication, and source history. |
| 37 | Analyze a personal dataset | Build | CSV and Excel parsing exists. Add secure personal-data intake and a deterministic analysis engine or controlled notebook connector. |
| 38 | Maintain a reading and newsletter digest | Verified | A provider-neutral Repeat case removed duplicate coverage, preserved links, adapted to a smaller attention budget, carried Coverage state, and suppressed a no-new-items update. Connected source intake remains separate. |
| 39 | Maintain an adaptive learning plan | Verified | A provider-neutral Repeat case preserved completed practice, used a failed assessment, and adapted the next plan to two current gaps. Test longer series before adding dedicated progress state. |
| 40 | Compare courses, programs, or credentials | Extend | Web research and document parsing cover comparison. Add option state, education portals, aid data, enrollment operations, and deadline reconciliation. |

### 5. Money, tax, insurance, benefits, and personal administration

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 41 | Maintain household cash flow and bills | Build | Add bank and biller connections, a cash-flow ledger, forecasting, shared-account reconciliation, and shortfall alerts. |
| 42 | Build an annual tax-readiness packet | Build | Public document parsing is insufficient. Add private tax-file intake, classification, missing-record checks, year evidence, accountant export, and filing controls. |
| 43 | Audit subscriptions | Extend | Historical receipt extraction passed. Add current mail proof, transaction feeds, a subscription register, cancellation operations, and billing-stop verification. |
| 44 | Review insurance coverage | Build | Policy parsing now helps. Add private policy intake, insurer and quote connections, normalized coverage, household risks, and qualified-review gates. |
| 45 | Reconcile an insurance claim | Build | Add durable claim state, private evidence intake, insurer connections, deadlines, payment reconciliation, appeal state, and external receipts. |
| 46 | Find and maintain benefits | Build | Add verified eligibility data, household definitions, evidence packages, portal uploads, application state, reporting duties, and recertification monitoring. |
| 47 | Consolidate retirement records | Build | Add plan-provider connections, private statement intake, account matching, fee comparisons, transfer tracking, tax evidence, and one-shot approvals. |
| 48 | Maintain credit and debt records | Build | Add bureau, lender, and bank connections, a debt ledger, private dispute evidence, status checks, and deadline alerts. |
| 49 | Maintain official documents and licenses | Extend | Repeat, web research, and public parsing cover tracking. Add identity-file intake, government portal uploads, confirmation checks, and household authority. |
| 50 | Escalate a consumer dispute | Extend | Task files, browsing, and review support a case. Add user evidence intake, complaint connections, delivery receipts, and durable case state. |
| 51 | Maintain an affairs and estate map | Build | Task directories are not a document vault. Add private intake, estate records, beneficiary roles, emergency access, update triggers, and secure export. |
| 52 | Administer a deceased person's accounts | Build | Add executor authority, death-certificate intake, jurisdiction rules, account and benefit connections, tax and property processes, notices, receipts, and reconciliation. |

### 6. Health and caregiving

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 53 | Consolidate a medical record | Build | Document parsing alone is insufficient. Add FHIR and portal connections, private record intake, normalization, conflict tracking, consent, and proxy access. |
| 54 | Maintain a medication and refill plan | Build | Add a verified medication record, pharmacy connections, refill events, reconciliation evidence, safety rules, and clinician confirmation. |
| 55 | Prepare a medical appointment brief | Extend | File parsing and synthesis support a draft. Add private health intake, portal data, symptom timelines, urgent routing, and a clinical acceptance test. |
| 56 | Coordinate referrals, tests, and specialists | Build | Add referral state, insurer and provider connections, record transfer, scheduling, transport, result retrieval, and follow-up tracking. |
| 57 | Monitor a care plan | Build | Add care-plan state, portal and device events, threshold rules, safe escalation, and loop closure. External event sources remain absent. |
| 58 | Compare providers or care services | Extend | Web research can create a shortlist. Add current network, cost, availability, accessibility, language, phone, and booking data. |
| 59 | Compare health plans | Build | Add private plan and formulary intake, network checks, care scenarios, deterministic cost modeling, tax rules, and professional review. |
| 60 | Build a health authorization or appeal packet | Build | Add durable case state, private denial and record intake, deadlines, portal uploads, submission receipts, status checks, and approvals. |
| 61 | Coordinate a safe care transition | Build | Add hospital and pharmacy connections, caregiver roles, warning escalation, equipment coordination, transport, and handoff confirmation. |
| 62 | Maintain a shared caregiver plan | Build | Add multiple humans, consent, caregiver roles, shared Tasks, assignments, handoffs, and notifications. Production still centers one local human. |
| 63 | Summarize care-recipient changes | Build | Add authorized health logs, recipient policy, change comparison, urgent routing, recipient-specific sharing, and delivery receipts. |
| 64 | Prepare a treatment decision brief | Extend | Research and file parsing support a draft. Add private medical intake, evidence filters, patient values, non-decision controls, and clinician review. |

### 7. Home, household, and consumer operations

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 65 | Maintain an asset and recall inventory | Build | Document parsing helps with receipts. Add private and image intake, OCR, an asset registry, owner matching, recall feeds, and history. |
| 66 | Run preventive home maintenance | Extend | Repeat supports schedules. Add an asset registry, maintenance rules, climate inputs, service history, and missed-schedule tests. |
| 67 | Coordinate a home repair project | Extend | Projects, Task files, research, and reviewed actions provide a base. Add bid intake, vendor checks, inspections, and payment milestones. |
| 68 | Optimize utilities and communication services | Extend | Public bill parsing can support analysis. Add private bill intake, utility tools, tariff normalization, renewal monitoring, and deterministic savings checks. |
| 69 | Maintain a meal and grocery plan | Extend | Add household food profiles, pantry state, grocery tools, nutrition constraints, allergy rules, substitutions, budgets, and attendance changes. |
| 70 | Coordinate the vehicle lifecycle | Extend | Add vehicle-record intake, recall feeds, maintenance tools, renewal checks, and tests for consequential actions. |
| 71 | Coordinate recurring pet care | Extend | Repeat provides scheduling. Add pet profiles, veterinary and pharmacy tools, licensing, insurer tools, medication safeguards, and caregiver handoffs. |
| 72 | Complete a return, warranty, or repair request | Extend | Public file downloads now work. Add retailer transfers, browser uploads, shipping workflows, deadline state, and remedy verification. |
| 73 | Manage recurring household services | Extend | Repeat provides scheduling. Add vendor records, contacts, payments, service history, backup-provider rules, and cancellation tests. |
| 74 | Maintain emergency readiness | Extend | Client notifications provide an alert path. Add local feeds, maps, contacts, supply inventory, private documents, reviews, and offline export. |

### 8. Travel, moves, housing, and events

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 75 | Build one live itinerary | Verified | Connected messages supplied two confirmations, one duplicate, and one later cancellation. The reviewed itinerary preserved references and missing evidence. |
| 76 | Plan and book a multi-leg trip | Extend | Browser routing improves recovery. Add travel inventory, booking tools, protected payment authority, receipt reconciliation, and partial-failure tests. |
| 77 | Monitor disruptions and prepare rebooking | Build | Implement external event sources, carrier feeds, rights rules, downstream dependency analysis, rebooking operations, and uncertain-outcome recovery. |
| 78 | Maintain international travel readiness | Extend | Public requirement downloads and parsing now work. Add private document intake, nationality rules, source freshness, medicine checks, and Calendar validation. |
| 79 | Track travel credits, refunds, and claims | Extend | Task files can hold evidence. Add loyalty and insurer tools, user uploads, deadline monitoring, value reconciliation, and partial-refund tests. |
| 80 | Coordinate group or accessible travel | Build | Add multi-human preferences, consent, conflict resolution, accessibility verification, shared approvals, and per-traveler documents. |
| 81 | Plan a personal event | Extend | Add contacts, invitations, RSVP state, vendor and payment tools, budgets, dependencies, and guest communications. |
| 82 | Run a move and propagate changes | Extend | Add mover, utility, bank, insurer, school, and government tools. Add private document transfer and confirmation reconciliation. |
| 83 | Maintain a housing search | Extend | Web and lease parsing provide a base. Add listing and map tools, commute calculations, fraud checks, applications, and hard-constraint tests. |
| 84 | Complete a trip or move closeout | Extend | Task files support a case record. Add transaction, private file, and photo intake with claim and deposit matching. |

### 9. Family, education, relationships, and community

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 85 | Maintain a family schedule and transport plan | Build | Add multi-human calendars, custody rules, shared transport resources, maps, backup plans, and collaborative assignment. |
| 86 | Turn school communications into a digest | Extend | Parsing can read supported files. Add school portal tools, private attachments, forms, child scopes, payments, event intake, and cross-source deduplication. |
| 87 | Coordinate childcare, camps, and activities | Extend | Add provider and waitlist tools, child health scopes, private uploads, payments, transport planning, schedule changes, and refund checks. |
| 88 | Balance household and care responsibilities | Build | Add shared ownership, workload views, consent, negotiation, and fairness controls. Collaboration and multi-user permissions remain excluded. |
| 89 | Run an education or scholarship campaign | Extend | Parsing supports source documents. Add application records, dependencies, recommendations, portal uploads, contributor roles, and deadline reconciliation. |
| 90 | Maintain family records and permission packets | Build | Add private file intake, family scopes, consent records, packet generation, expiry checks, secure sharing, and access audit. |
| 91 | Coordinate community or volunteer participation | Extend | Public forms can be downloaded and parsed. Add uploads, training state, communications, renewal reconciliation, and current registration tests. |
| 92 | Maintain a relationship and occasion plan | Extend | Add contact and relationship scopes, occasion data, shared duties, shopping, messaging tools, and cultural-preference tests. |

### 10. Digital life, privacy, records, and resilience

| # | Task | Status | Noema gap or improvement |
| ---: | --- | --- | --- |
| 93 | Organize and preserve files, email, and photos | Build | Working directories and public downloads are not an archive. Add private and cloud intake, photo support, deduplication, migration, backups, and restore checks. |
| 94 | Maintain an account and security inventory | Build | Add account metadata, password-manager status, and device-security integrations. Keep credentials outside model context and verify each change. |
| 95 | Coordinate account or identity-theft recovery | Build | Add a recovery workspace, trusted-device checks, bank and bureau tools, private evidence intake, dispute tracking, and continuous monitoring. |
| 96 | Review privacy settings and data exposure | Build | Browser routes can reach public sites, but profiles remain temporary. Add account inventory, authenticated access, deletion tracking, and reappearance monitoring. |
| 97 | Complete device replacement and migration | Build | Add device, backup, authenticator, password-manager, migration, verification, and disposal integrations. Noema cannot control personal devices. |
| 98 | Maintain a personal knowledge base | Verified | Normal Memory consolidation replaced one fact, preserved duplicate direct evidence and superseded values, and supported later retrieval. Direct editing remains separate. |
| 99 | Prepare a digital-legacy plan | Build | Add digital-legacy records, trusted roles, delegated access, estate scopes, platform actions, periodic review, and secure export. |
| 100 | Make digital information accessible | Extend | Document parsing now converts supported files to Markdown. Add user file intake, OCR, accessible exports, validation, profiles, and affected-user tests. |

## Cross-cutting findings

### 1. The task is usually a persistent loop

Most tasks repeat one loop: discover, normalize, decide, act, monitor, reconcile, and report.

A draft, booking, form, or reminder is often only an intermediate result.

### 2. Personal context is operational

Useful context includes budgets, working hours, relationships, care needs, accessibility, risk tolerance, travel pace, and prior choices.

The assistant must know which facts remain current. It must also detect changes and conflicting instructions.

### 3. Reconciliation creates much of the value

Important records often exist in several formats and systems.

The assistant must preserve source history while it matches people, accounts, dates, transactions, commitments, and document versions.

### 4. Monitoring must be selective

Long-duration tasks can create notification noise.

Good monitoring needs a baseline, a material-change rule, an escalation threshold, and a clear stop condition.

### 5. Exceptions determine real reliability

Normal paths are often simple. Identity checks, missing documents, denials, cancellations, changed rules, and partial failures create the difficult work.

The assistant must retain state and offer safe recovery paths after an interruption.

### 6. Consequence changes the completion standard

For low-risk work, a useful answer can be sufficient.

For high-risk work, completion needs verified evidence, explicit authority, professional review when necessary, and a receipt for every external change.

### 7. Common does not mean universal

Some tasks occur daily across a broad population. Other tasks recur intensely during caregiving, job search, illness, travel, education, or moving.

An assistant needs both routine support and strong support for major life events.

## Safety and authority

These tasks describe outcomes. They do not grant unrestricted access or action authority.

An implementation should apply these controls:

- Keep credentials outside model context and ordinary logs.
- Limit private information to the authorized person, purpose, source, and time.
- Confirm consequential messages, purchases, bookings, cancellations, filings, transfers, deletions, account closures, bulk changes, and device disposal unless prior policy permits them.
- Require a verified backup before destructive data or device actions. Retain a change receipt after each action.
- Show evidence, assumptions, uncertainty, and missing records before a high-stakes decision.
- Preserve the original record when normalization or redaction creates a derivative.
- Use qualified professionals for diagnosis, legal judgment, regulated financial advice, and other reserved decisions.
- Stop or escalate when identity, consent, authority, or external outcome is unclear.

## Limitations

This report does not estimate the exact frequency of each task.

The sources use different populations, countries, methods, dates, and definitions.

The CFPB estimate represents adults with a nationwide credit-report record and their households.

Official process guides show workflow complexity. They do not measure how often every step causes difficulty.

Vendor surveys provide useful workplace evidence. Their populations and products can limit generalization.

Several task families depend on local law, benefits, health systems, and service availability.

Digital exclusion can prevent the workflows that this report assumes.

Agent benchmark scores change quickly. The cited results describe the original studies.

The list omits many specialist tasks. It favors tasks with broad personal-assistant value and reusable difficulty patterns.

## Selected source ledger

### Activity, administration, and work

| Source | Publisher and date | Contribution |
| --- | --- | --- |
| [American Time Use Survey, 2024 Results](https://www.bls.gov/news.release/archives/atus_06262025.htm) | U.S. Bureau of Labor Statistics, 2025 | Household work, purchases, care, education, communication, and work activity |
| [OECD Time Use Database](https://www.oecd.org/en/data/datasets/time-use-database.html) | OECD, updated 2026 | Comparable paid, unpaid, care, study, and personal time across 30 countries |
| [Time Use in the UK: March 2023](https://www.ons.gov.uk/peoplepopulationandcommunity/personalandhouseholdfinances/incomeandwealth/bulletins/timeuseintheuk/march2023) | UK Office for National Statistics, 2023 | Household administration, care, shopping, and digital-device use |
| [OECD Employment Outlook 2025](https://www.oecd.org/en/publications/oecd-employment-outlook-2025_194a947b-en/full-report/component-6.html) | OECD, 2025 | Unpaid care and hidden cognitive domestic work |
| [Online public-service use in 2024](https://ec.europa.eu/eurostat/web/products-eurostat-news/w/ddn-20250226-1) | Eurostat, 2025 | Information, forms, personal records, tax, appointments, claims, and benefits |
| [eGovernment Benchmark 2026](https://digital-strategy.ec.europa.eu/en/library/digital-decade-2026-egovernment-benchmark-2026) | European Commission, 2026 | Ninety-six public services across nine life events |
| [Administrative Burden](https://www.gao.gov/products/gao-25-107239) | U.S. Government Accountability Office, 2025 | Eligibility research, applications, supporting records, and missed benefits |
| [Will AI Fix Work?](https://www.microsoft.com/en-us/worklab/work-trend-index/will-ai-fix-work) | Microsoft WorkLab, 2023 | Communication load, search burden, meeting friction, and focus loss |
| [Email overload and commitment detection](https://www.microsoft.com/en-us/research/blog/email-overload-using-machine-learning-to-manage-messages-commitments/) | Microsoft Research, 2019 | Email deferral, implicit commitments, priority, and follow-up |
| [Calendar.help](https://www.microsoft.com/en-us/research/publication/calendar-help-designing-workflow-based-scheduling-agent-humans-loop/) | Microsoft Research and CHI, 2017 | Delegated scheduling, time zones, negotiation, and exception handling |
| [Executive Administrative Assistant](https://www.onetonline.org/link/summary/43-6011.00) | O*NET OnLine, updated 2026 | Scheduling, correspondence, research, records, reports, agendas, and policy interpretation |
| [Anatomy of Work Global Index 2023](https://investors.asana.com/news-releases/news-release-details/asana-anatomy-work-global-index-2023-smart-collaboration-and/) | Asana and GWI, 2023 | Coordination work across 9,615 knowledge workers |
| [WorkArena](https://arxiv.org/abs/2403.07718) | ServiceNow Research, 2024 | Common knowledge work inside enterprise software |
| [Job Search](https://www.careeronestop.org/JobSearch/job-search.aspx) | U.S. Department of Labor partner service | Research, networking, applications, interviews, and negotiation |
| [Job-search methods, 2024](https://www.bls.gov/cps/data/aa2024/cpsaat34.htm) | U.S. Bureau of Labor Statistics, 2025 | Multiple simultaneous methods used by job seekers |
| [Systematic-review labor study](https://doi.org/10.1136/bmjopen-2016-012545) | BMJ Open, 2017 | Long duration, large searches, screening, and multi-person evidence work |
| [Trends in Adult Learning](https://www.oecd.org/en/publications/trends-in-adult-learning_ec0624a6-en.html) | OECD, 2025 | Adult-learning participation, time barriers, and access constraints |
| [A Causal Test of the Strength of Weak Ties](https://doi.org/10.1126/science.abl4476) | Science, 2022 | Relationship strength and job mobility across large professional networks |
| [Parent and Family Involvement in Education: 2023](https://nces.ed.gov/pubs2024/2024113.pdf) | U.S. National Center for Education Statistics, 2024 | School messages, events, meetings, conferences, and homework support |
| [Evaluate Financial Aid Offers](https://studentaid.gov/articles/evaluating-financial-aid-offers/) | Federal Student Aid | Cost, grants, debt, deadlines, and nonfinancial preferences |

### Money, care, home, and life events

| Source | Publisher and date | Contribution |
| --- | --- | --- |
| [Economic Well-Being of U.S. Households in 2024](https://www.federalreserve.gov/publications/2025-economic-well-being-of-us-households-in-2024-income-and-expenses.htm) | Federal Reserve Board, 2025 | Income variation, expenses, bills, savings, credit, and financial shocks |
| [Making Ends Meet in 2024](https://www.consumerfinance.gov/data-research/research-reports/making-ends-meet-in-2024-insights-from-the-making-ends-meet-survey/) | Consumer Financial Protection Bureau, 2024 | Household bill and expense difficulty |
| [2025 Form 1040 Instructions](https://www.irs.gov/pub/irs-prior/i1040gi--2025.pdf) | Internal Revenue Service, 2025 | Recordkeeping, planning, form completion, submission, time, and cost |
| [401(k) Account Tracking and Consolidation](https://www.gao.gov/products/gao-24-103577) | U.S. Government Accountability Office, 2024 | Old-account discovery, rollover steps, unclear processes, and consolidation |
| [Bill Calendar](https://www.consumerfinance.gov/archive/blog/budget-help-manage-your-monthly-expenses-bill-calendar/) | Consumer Financial Protection Bureau, 2019 | Due-date tracking, expense planning, and shared budgeting |
| [Retirement Plan Guide](https://www.dol.gov/agencies/ebsa/about-ebsa/our-activities/resource-center/publications/what-you-should-know-about-your-retirement-plan) | U.S. Department of Labor | Statements, vesting, distributions, and rollovers |
| [Subscription and auto-renewal guide](https://consumer.ftc.gov/articles/getting-and-out-free-trials-auto-renewals-and-negative-option-subscriptions) | U.S. Federal Trade Commission, 2024 | Terms, renewals, cancellation proof, billing checks, and disputes |
| [Resolving problems with a business](https://consumer.ftc.gov/articles/solving-problems-business-returns-refunds-and-other-resolutions) | U.S. Federal Trade Commission | Receipts, warranties, contracts, requested remedies, and escalation |
| [How Insurance Works](https://content.naic.org/consumer/how-does-insurance-work) | National Association of Insurance Commissioners | Insurance types, risk, coverage, premiums, claims, and government marketplaces |
| [Caregiving in the US 2025](https://www.caregivingintheus.org/report_categories/caregiving-in-the-us-report-2025-downloads/) | AARP and National Alliance for Caregiving, 2025 | Care prevalence, duration, medical tasks, finances, and coordination |
| [Care Coordination Measures Atlas](https://www.ahrq.gov/ncepcr/care/coordination/atlas/chapter6.html) | U.S. Agency for Healthcare Research and Quality | Accountability, transfer, transitions, goals, planning, and monitoring |
| [Patient portals and caregiver access, 2024](https://healthit.gov/data/data-briefs/individuals-access-and-use-patient-portals-and-smartphone-health-apps-2024/) | U.S. Office of the National Coordinator, 2025 | Patient records, proxy access, downloads, and health applications |
| [Health-plan total costs](https://www.healthcare.gov/choose-a-plan/your-total-costs/) | HealthCare.gov | Premiums, deductibles, cost sharing, expected use, and annual estimates |
| [Discharge Planning Checklist](https://www.medicare.gov/publications/11376-your-discharge-planning-checklist.pdf) | Medicare | Equipment, medicines, appointments, home tasks, transport, costs, and support |
| [Your Medicine: Be Smart. Be Safe.](https://www.ahrq.gov/questions/resources/your-meds/index.html) | U.S. Agency for Healthcare Research and Quality | Medication lists, questions, records, pharmacies, and trusted support |
| [Consumer recall resources](https://www.cpsc.gov/About-CPSC/Consumer-Resources) | U.S. Consumer Product Safety Commission | Recall monitoring, repair, refund, replacement, and product records |
| [Emergency Financial First Aid Kit](https://www.ready.gov/sites/default/files/2020-03/ready_emergency-financial-first-aid-toolkit-checklists-and-forms.pdf) | Ready.gov and FEMA | Critical household, financial, medical, property, and pet records |
| [Getting Your Affairs in Order](https://www.nia.nih.gov/health/advance-care-planning/getting-your-affairs-order-checklist-documents-prepare-future) | National Institute on Aging, reviewed 2023 | Estate, finance, health, permission, trusted-person, and document planning |
| [Report a Death](https://www.usa.gov/report-a-death) | USAGov | Benefits, taxes, passports, elections, credit reporting, and identity-theft prevention |
| [International Travel Checklist](https://travel.state.gov/en/international-travel/planning/checklist.html) | U.S. Department of State, updated 2025 | Entry, passport, visa, health, medicine, insurance, and child documents |
| [Airline Cancellation and Delay Dashboard](https://www.transportation.gov/airconsumer/airline-cancellation-delay-dashboard) | U.S. Department of Transportation | Rebooking, meals, lodging, transport, and airline commitments |
| [Geographic Mobility at a Glance](https://www.census.gov/topics/population/migration/guidance/acs-1yr.html) | U.S. Census Bureau, revised 2025 | Population moving rates and affected households |
| [Change of Address](https://www.usa.gov/change-address) | USAGov, updated 2026 | Mail, tax, benefits, immigration, vehicle, license, and voting changes |

### Digital resilience and agent difficulty

| Source | Publisher and date | Contribution |
| --- | --- | --- |
| [How Americans View Data Privacy](https://www.pewresearch.org/internet/2023/10/18/how-americans-view-data-privacy/) | Pew Research Center, 2023 | Password overload, privacy choices, breaches, fraud, and account takeovers |
| [Secure Our World](https://www.cisa.gov/secure-our-world) | U.S. Cybersecurity and Infrastructure Security Agency | Passwords, multifactor authentication, phishing, and software updates |
| [Identity-theft recovery guide](https://consumer.ftc.gov/articles/what-know-about-identity-theft) | U.S. Federal Trade Commission | Credit freezes, alerts, reports, disputes, letters, and recovery plans |
| [Personal Digital Archiving](https://www.digitalpreservation.gov/personalarchiving/) | Library of Congress | Email, photos, media, records, websites, formats, descriptions, and storage |
| [How People with Disabilities Use the Web](https://www.w3.org/WAI/people-use-web/) | W3C Web Accessibility Initiative | Digital barriers, assistive tools, user needs, and diverse abilities |
| [Accessible PDFs](https://www.section508.gov/create/pdfs/) | U.S. General Services Administration | Document testing, remediation, accessible formats, and manual checks |
| [AssistantBench](https://aclanthology.org/2024.emnlp-main.505/) | ACL and EMNLP, 2024 | Realistic, time-consuming, multi-site web tasks |
| [GAIA](https://proceedings.iclr.cc/paper_files/paper/2024/hash/25ae35b5b1738d80f1f03a8713e405ec-Abstract-Conference.html) | ICLR, 2024 | Reasoning, browsing, multimodal evidence, and tool use |
| [OSWorld](https://papers.nips.cc/paper_files/paper/2024/hash/5d413e48f84dc61244b6be550f1cd8f5-Abstract-Datasets_and_Benchmarks_Track.html) | NeurIPS, 2024 | Real web, desktop, file, and multi-application workflows |
| [LongMemEval](https://arxiv.org/abs/2410.10813) | ICLR, 2025 | Multi-session memory, temporal reasoning, updates, and abstention |
| [TravelPlanner](https://iclr.cc/virtual/2024/22166) | ICLR, 2024 | Long-duration, multi-constraint planning across a large travel database |
| [COMPASS](https://machinelearning.apple.com/research/multi-turn-benchmark) | Apple Machine Learning Research, 2025 | Preference optimization and coordination across several travel services |
