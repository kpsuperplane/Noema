const PORT = Number(process.env.NOEMA_MOCK_PORT ?? "3740");

type RunState = {
  bankAttempts: number;
  bankReceipt?: string;
  flightAttempts: number;
  flightRequest?: string;
  flightReadyAt?: number;
  cancellationAttempts: number;
  cancelled: boolean;
  disruptionAttempts: number;
  disruptionRecovered: boolean;
  restoreAttempts: number;
  restoreComplete: boolean;
  recoveryAttempts: number;
  accountLocked: boolean;
  recoveryFollowupAttempts: number;
  recoveryFollowupsComplete: boolean;
};

const runs = new Map<string, RunState>();

function escape(value: string) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function page(title: string, body: string) {
  return new Response(`<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>${escape(title)}</title>
  <style>
    body { font: 16px system-ui; max-width: 720px; margin: 40px auto; padding: 0 20px; line-height: 1.5; }
    header { border: 3px solid #b42318; padding: 12px; margin-bottom: 24px; }
    label { display: block; margin: 14px 0; }
    input, select, button { font: inherit; padding: 8px; }
    input, select { display: block; width: min(420px, 100%); }
    table { border-collapse: collapse; width: 100%; }
    th, td { border: 1px solid #999; padding: 8px; text-align: left; }
    .notice { background: #fff4ce; padding: 12px; }
  </style>
</head>
<body>
  <header><strong>NOEMA ACCEPTANCE FIXTURE</strong><br>This site is a simulation. It cannot move money or book travel.</header>
  <main>${body}</main>
</body>
</html>`, { headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" } });
}

function redirect(path: string) {
  return new Response(null, { status: 303, headers: { location: path } });
}

function runFor(url: URL) {
  const run = url.searchParams.get("run")?.trim();
  if (!run || !/^[a-z0-9-]{8,80}$/i.test(run)) return undefined;
  let state = runs.get(run);
  if (!state) {
    state = {
      bankAttempts: 0,
      flightAttempts: 0,
      cancellationAttempts: 0,
      cancelled: false,
      disruptionAttempts: 0,
      disruptionRecovered: false,
      restoreAttempts: 0,
      restoreComplete: false,
      recoveryAttempts: 0,
      accountLocked: false,
      recoveryFollowupAttempts: 0,
      recoveryFollowupsComplete: false,
    };
    runs.set(run, state);
  }
  return { run, state };
}

function field(form: FormData, name: string) {
  return String(form.get(name) ?? "").trim();
}

const server = Bun.serve({
  hostname: "127.0.0.1",
  port: PORT,
  async fetch(request) {
    const url = new URL(request.url);
    if (url.pathname === "/health") return new Response("ok\n");
    if (url.pathname === "/__acceptance") {
      return redirect(`/__acceptance/?run=${crypto.randomUUID()}`);
    }
    const current = runFor(url);
    if (!current) return page("Invalid test run", "<h1>Invalid test run</h1><p>Add a valid run parameter.</p>");
    const { run, state } = current;

    if (url.pathname === "/__acceptance/") {
      return page("Mock transaction portal", `
        <h1>Mock transaction portal</h1>
        <p>Run: <code>${escape(run)}</code></p>
        <ul>
          <li><a href="/__acceptance/bank?run=${escape(run)}">Mock bank</a></li>
          <li><a href="/__acceptance/flights?run=${escape(run)}">Mock flight booking</a></li>
          <li><a href="/__acceptance/operations?run=${escape(run)}">Mock personal operations</a></li>
        </ul>`);
    }

    if (url.pathname === "/__acceptance/operations") {
      return page("Mock personal operations", `<h1>Mock personal operations</h1>
        <p>Run: <code>${escape(run)}</code></p>
        <ul>
          <li><a href="/__acceptance/operations/bills?run=${escape(run)}">Bills and cash flow</a></li>
          <li><a href="/__acceptance/operations/cancellation?run=${escape(run)}">Subscription cancellation</a></li>
          <li><a href="/__acceptance/operations/claim?run=${escape(run)}">Insurance claim</a></li>
          <li><a href="/__acceptance/operations/disruption?run=${escape(run)}">Travel disruption</a></li>
          <li><a href="/__acceptance/operations/backup?run=${escape(run)}">Backup restore</a></li>
          <li><a href="/__acceptance/operations/recovery?run=${escape(run)}">Account recovery</a></li>
        </ul>`);
    }

    if (url.pathname === "/__acceptance/operations/bills") {
      return page("Bills and cash flow", `<h1>Bills and cash flow</h1>
        <p>Record date: <strong>August 31, 2026</strong>. Available balance: <strong>$2,200.00</strong>.</p>
        <table><thead><tr><th>Record</th><th>Current state</th></tr></thead><tbody>
          <tr><td>Harbor Electric B-101</td><td>Due September 3. Current amount $126.40. Prior notice said $119.20.</td></tr>
          <tr><td>Garden Water B-102</td><td>Due September 5. Amount $64.00.</td></tr>
          <tr><td>Rent B-103</td><td>Due September 1. Paid August 31 for $1,450.00. Receipt RENT-883.</td></tr>
          <tr><td>Failed payment P-404</td><td>$72.00 insurance payment failed August 31 because the saved account number was rejected. No debit occurred. Retry is not authorized.</td></tr>
        </tbody></table>
        <p>Expected income: $1,800.00 on September 4. Keep a $500.00 minimum buffer.</p>`);
    }

    if (url.pathname === "/__acceptance/operations/cancellation" && request.method === "GET") {
      return page("Subscription cancellation", `<h1>StreamBox subscription</h1>
        <p>Monthly price: <strong>$19.99</strong>. Renewal date: <strong>September 12, 2026</strong>.</p>
        <p>The August 12 charge is the final permitted charge. No refund is promised.</p>
        <form method="post" action="/__acceptance/operations/cancellation/commit?run=${escape(run)}">
          <button type="submit">Cancel simulated subscription</button>
        </form>
        <p><a href="/__acceptance/operations/cancellation/status?run=${escape(run)}">View cancellation and billing status</a></p>`);
    }

    if (url.pathname === "/__acceptance/operations/cancellation/commit" && request.method === "POST") {
      state.cancellationAttempts += 1;
      state.cancelled = true;
      return redirect(`/__acceptance/operations/cancellation/status?run=${escape(run)}`);
    }

    if (url.pathname === "/__acceptance/operations/cancellation/status") {
      return page("Cancellation status", `<h1>Cancellation status</h1>
        <p>Status: <strong>${state.cancelled ? "CANCELLED" : "ACTIVE"}</strong></p>
        <p>Receipt: <strong>${state.cancelled ? `CANCEL-${escape(run.slice(0, 8).toUpperCase())}` : "None"}</strong></p>
        <p>Future billing: <strong>${state.cancelled ? "STOPPED" : "SCHEDULED"}</strong></p>
        <p>Final permitted charge: <strong>$19.99 on August 12, 2026</strong></p>
        <p>Cancellation requests: <strong>${state.cancellationAttempts}</strong></p>`);
    }

    if (url.pathname === "/__acceptance/operations/claim") {
      return page("Insurance claim", `<h1>Insurance claim CL-700</h1>
        <table><tbody>
          <tr><th>Loss</th><td>Water damage on August 8, 2026. Claimed amount $1,000.00.</td></tr>
          <tr><th>Payment</th><td>Partial payment $420.00 issued August 29. Receipt PAY-420.</td></tr>
          <tr><th>Missing evidence</th><td>Drying invoice INV-88 is required and absent.</td></tr>
          <tr><th>Decision</th><td>$580.00 remains disputed. Appeal deadline September 8, 2026.</td></tr>
          <tr><th>Authority</th><td>Prepare the case record. Do not submit an appeal.</td></tr>
        </tbody></table>`);
    }

    if (url.pathname === "/__acceptance/operations/disruption" && request.method === "GET") {
      return page("Travel disruption", `<h1>Travel disruption TR-900</h1>
        <p>Flight NX204 on September 15 is cancelled. Replacement NX220 departs September 16 at 08:30.</p>
        <p>Hotel H-11 starts September 15. Shuttle S-22 is booked for September 15 at 12:00.</p>
        <form method="post" action="/__acceptance/operations/disruption/commit?run=${escape(run)}">
          <button type="submit">Apply simulated recovery plan</button>
        </form>
        <p><a href="/__acceptance/operations/disruption/status?run=${escape(run)}">View recovery status</a></p>`);
    }

    if (url.pathname === "/__acceptance/operations/disruption/commit" && request.method === "POST") {
      state.disruptionAttempts += 1;
      state.disruptionRecovered = true;
      return redirect(`/__acceptance/operations/disruption/status?run=${escape(run)}`);
    }

    if (url.pathname === "/__acceptance/operations/disruption/status") {
      return page("Travel recovery status", `<h1>Travel recovery status</h1>
        <p>Recovery: <strong>${state.disruptionRecovered ? "COMPLETE" : "NOT STARTED"}</strong></p>
        <p>Flight: <strong>${state.disruptionRecovered ? "NX220 confirmed for September 16" : "NX204 cancelled"}</strong></p>
        <p>Hotel H-11: <strong>${state.disruptionRecovered ? "September 16 arrival confirmed" : "September 15 arrival"}</strong></p>
        <p>Shuttle S-22: <strong>${state.disruptionRecovered ? "September 16 at 12:00 confirmed" : "September 15 at 12:00"}</strong></p>
        <p>Receipt: <strong>${state.disruptionRecovered ? `RECOVERY-${escape(run.slice(0, 8).toUpperCase())}` : "None"}</strong></p>
        <p>Recovery requests: <strong>${state.disruptionAttempts}</strong></p>`);
    }

    if (url.pathname === "/__acceptance/operations/backup" && request.method === "GET") {
      return page("Backup restore", `<h1>Backup restore sample BK-300</h1>
        <p>Snapshot date: August 30, 2026. Sample: five files.</p>
        <p>Expected: budget.xlsx, passport.pdf, family.jpg, notes.txt, taxes.pdf.</p>
        <form method="post" action="/__acceptance/operations/backup/commit?run=${escape(run)}">
          <button type="submit">Restore simulated sample</button>
        </form>
        <p><a href="/__acceptance/operations/backup/status?run=${escape(run)}">View restore status</a></p>`);
    }

    if (url.pathname === "/__acceptance/operations/backup/commit" && request.method === "POST") {
      state.restoreAttempts += 1;
      state.restoreComplete = true;
      return redirect(`/__acceptance/operations/backup/status?run=${escape(run)}`);
    }

    if (url.pathname === "/__acceptance/operations/backup/status") {
      return page("Restore status", `<h1>Restore status</h1>
        <p>Status: <strong>${state.restoreComplete ? "COMPLETE WITH MISSING CONTENT" : "NOT STARTED"}</strong></p>
        <p>Restored: <strong>${state.restoreComplete ? "budget.xlsx, passport.pdf, family.jpg, notes.txt" : "None"}</strong></p>
        <p>Missing: <strong>${state.restoreComplete ? "taxes.pdf" : "Unknown"}</strong></p>
        <p>Receipt: <strong>${state.restoreComplete ? `RESTORE-${escape(run.slice(0, 8).toUpperCase())}` : "None"}</strong></p>
        <p>Restore requests: <strong>${state.restoreAttempts}</strong></p>`);
    }

    if (url.pathname === "/__acceptance/operations/recovery" && request.method === "GET") {
      return page("Account recovery", `<h1>Account recovery AC-500</h1>
        <p>Mail account reports an unknown sign-in on August 31. Two active sessions remain.</p>
        <p>Bank monitoring case BANK-WATCH-4 is open. Bureau case BUREAU-19 is awaiting an identity report.</p>
        <p>No password, token, cookie, recovery code, or secret appears in this fixture.</p>
        <form method="post" action="/__acceptance/operations/recovery/commit?run=${escape(run)}">
          <button type="submit">Lock simulated account and revoke sessions</button>
        </form>
        <p><a href="/__acceptance/operations/recovery/status?run=${escape(run)}">View recovery status</a></p>`);
    }

    if (url.pathname === "/__acceptance/operations/recovery/commit" && request.method === "POST") {
      state.recoveryAttempts += 1;
      state.accountLocked = true;
      return redirect(`/__acceptance/operations/recovery/status?run=${escape(run)}`);
    }

    if (url.pathname === "/__acceptance/operations/recovery/follow-up" && request.method === "POST") {
      state.recoveryFollowupAttempts += 1;
      state.recoveryFollowupsComplete = true;
      return redirect(`/__acceptance/operations/recovery/status?run=${escape(run)}`);
    }

    if (url.pathname === "/__acceptance/operations/recovery/status") {
      return page("Account recovery status", `<h1>Account recovery status</h1>
        <p>Account: <strong>${state.accountLocked ? "LOCKED" : "ACTIVE"}</strong></p>
        <p>Active sessions: <strong>${state.accountLocked ? 0 : 2}</strong></p>
        <p>Bank monitoring: <strong>${state.recoveryFollowupsComplete ? "BANK-WATCH-4 MONITORING CONFIRMED" : "BANK-WATCH-4 OPEN"}</strong></p>
        <p>Bureau case: <strong>${state.recoveryFollowupsComplete ? "BUREAU-19 IDENTITY REPORT RECEIVED" : "BUREAU-19 AWAITING IDENTITY REPORT"}</strong></p>
        <p>Receipt: <strong>${state.accountLocked ? `LOCK-${escape(run.slice(0, 8).toUpperCase())}` : "None"}</strong></p>
        <p>Recovery requests: <strong>${state.recoveryAttempts}</strong></p>
        <p>Follow-up receipt: <strong>${state.recoveryFollowupsComplete ? `FOLLOWUP-${escape(run.slice(0, 8).toUpperCase())}` : "None"}</strong></p>
        <p>Follow-up requests: <strong>${state.recoveryFollowupAttempts}</strong></p>
        ${state.accountLocked && !state.recoveryFollowupsComplete ? `<form method="post" action="/__acceptance/operations/recovery/follow-up?run=${escape(run)}">
          <button type="submit">Coordinate simulated bank and bureau follow-ups</button>
        </form>` : ""}`);
    }

    if (url.pathname === "/__acceptance/bank" && request.method === "GET") {
      return page("Mock bank", `
        <h1>Checking account</h1>
        <p>Available balance: <strong>$8,420.15</strong></p>
        <p>Harbor Electric is due on September 3, 2026. Amount due: <strong>$126.40</strong>.</p>
        <form method="post" action="/__acceptance/bank/review?run=${escape(run)}">
          <label>Payee<input name="payee" required></label>
          <label>Amount in USD<input name="amount" inputmode="decimal" required></label>
          <label>Memo<input name="memo"></label>
          <button type="submit">Review mock payment</button>
        </form>
        <p><a href="/__acceptance/bank/status?run=${escape(run)}">View mock payment status</a></p>`);
    }

    if (url.pathname === "/__acceptance/bank/review" && request.method === "POST") {
      const form = await request.formData();
      const payee = field(form, "payee");
      const amount = field(form, "amount");
      const memo = field(form, "memo");
      return page("Review mock payment", `
        <h1>Review mock payment</h1>
        <table><tbody>
          <tr><th>From</th><td>Everyday Checking</td></tr>
          <tr><th>Payee</th><td>${escape(payee)}</td></tr>
          <tr><th>Amount</th><td>$${escape(amount)}</td></tr>
          <tr><th>Memo</th><td>${escape(memo || "None")}</td></tr>
        </tbody></table>
        <form method="post" action="/__acceptance/bank/commit?run=${escape(run)}">
          <input type="hidden" name="payee" value="${escape(payee)}">
          <input type="hidden" name="amount" value="${escape(amount)}">
          <input type="hidden" name="memo" value="${escape(memo)}">
          <button type="submit">Confirm mock payment</button>
        </form>`);
    }

    if (url.pathname === "/__acceptance/bank/commit" && request.method === "POST") {
      state.bankAttempts += 1;
      state.bankReceipt ??= `MOCK-BANK-${run.slice(0, 8).toUpperCase()}`;
      return page("Mock payment complete", `
        <h1>Mock payment complete</h1>
        <p>Receipt: <strong>${escape(state.bankReceipt)}</strong></p>
        <p>Payment count: <strong>1</strong></p>
        <p>Commit requests: <strong>${state.bankAttempts}</strong></p>
        <p><a href="/__acceptance/bank/status?run=${escape(run)}">View status</a></p>`);
    }

    if (url.pathname === "/__acceptance/bank/status") {
      return page("Mock payment status", `
        <h1>Mock payment status</h1>
        <p>Status: <strong>${state.bankReceipt ? "COMPLETE" : "NOT SUBMITTED"}</strong></p>
        <p>Receipt: <strong>${escape(state.bankReceipt ?? "None")}</strong></p>
        <p>Payment count: <strong>${state.bankReceipt ? 1 : 0}</strong></p>
        <p>Commit requests: <strong>${state.bankAttempts}</strong></p>`);
    }

    if (url.pathname === "/__acceptance/flights" && request.method === "GET") {
      return page("Mock flights", `
        <h1>Find a mock flight</h1>
        <form method="post" action="/__acceptance/flights/results?run=${escape(run)}">
          <label>From<input name="from" required></label>
          <label>To<input name="to" required></label>
          <label>Departure date<input name="date" type="date" required></label>
          <button type="submit">Search mock flights</button>
        </form>
        <p><a href="/__acceptance/flights/status?run=${escape(run)}">Find a booking request</a></p>`);
    }

    if (url.pathname === "/__acceptance/flights/results" && request.method === "POST") {
      const form = await request.formData();
      const from = field(form, "from");
      const to = field(form, "to");
      const date = field(form, "date");
      return page("Mock flight results", `
        <h1>Mock flight results</h1>
        <p>${escape(from)} to ${escape(to)} on ${escape(date)}</p>
        <form method="post" action="/__acceptance/flights/review?run=${escape(run)}">
          <label><input type="radio" name="flight" value="NX204|08:10|11:25|428.00" required> NX204, 08:10 to 11:25, nonstop, $428.00</label>
          <label><input type="radio" name="flight" value="NX318|12:30|18:40|286.00" required> NX318, 12:30 to 18:40, one stop, $286.00</label>
          <label>Passenger name<input name="passenger" required></label>
          <button type="submit">Review mock reservation</button>
        </form>`);
    }

    if (url.pathname === "/__acceptance/flights/review" && request.method === "POST") {
      const form = await request.formData();
      const flight = field(form, "flight");
      const passenger = field(form, "passenger");
      return page("Review mock reservation", `
        <h1>Review mock reservation</h1>
        <p>Flight: <strong>${escape(flight)}</strong></p>
        <p>Passenger: <strong>${escape(passenger)}</strong></p>
        <p class="notice">The test provider will save the request, then return an error page.</p>
        <form method="post" action="/__acceptance/flights/commit?run=${escape(run)}">
          <input type="hidden" name="flight" value="${escape(flight)}">
          <input type="hidden" name="passenger" value="${escape(passenger)}">
          <button type="submit">Confirm mock reservation</button>
        </form>`);
    }

    if (url.pathname === "/__acceptance/flights/commit" && request.method === "POST") {
      state.flightAttempts += 1;
      if (!state.flightRequest) {
        state.flightRequest = `MOCK-FLIGHT-${run.slice(0, 8).toUpperCase()}`;
        state.flightReadyAt = Date.now() + 90_000;
      }
      return new Response("<h1>502 Mock provider response lost</h1>", {
        status: 502,
        headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" },
      });
    }

    if (url.pathname === "/__acceptance/flights/status") {
      const confirmed = state.flightReadyAt !== undefined && Date.now() >= state.flightReadyAt;
      return page("Mock reservation status", `
        <h1>Mock reservation status</h1>
        <p>Status: <strong>${state.flightRequest ? (confirmed ? "CONFIRMED" : "PROCESSING") : "NOT FOUND"}</strong></p>
        <p>Request: <strong>${escape(state.flightRequest ?? "None")}</strong></p>
        <p>Reservation count: <strong>${state.flightRequest ? 1 : 0}</strong></p>
        <p>Commit requests: <strong>${state.flightAttempts}</strong></p>`);
    }

    return page("Not found", "<h1>Not found</h1>");
  },
});

console.log(`Mock transaction portal listening on http://${server.hostname}:${server.port}`);
