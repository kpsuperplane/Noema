const PORT = Number(process.env.NOEMA_MOCK_PORT ?? "3740");

type RunState = {
  bankAttempts: number;
  bankReceipt?: string;
  flightAttempts: number;
  flightRequest?: string;
  flightReadyAt?: number;
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
    state = { bankAttempts: 0, flightAttempts: 0 };
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
        </ul>`);
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
