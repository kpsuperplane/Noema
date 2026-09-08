#!/usr/bin/env bun

// Synthetic mixed-record inventory service for PA-036.
// It has no connection to a retailer, manufacturer, mailbox, or real account.

const port = Number(process.env.NOEMA_INVENTORY_PORT ?? "3757");
const fixtureVersion = "2026-09-08-inventory-api-v1";

type RecordFormat = "pdf" | "image" | "email" | "spreadsheet";

type InventoryRecord = {
  record_id: string;
  format: RecordFormat;
  source_locator: string;
  item: string;
  manufacturer: string;
  model: string;
  serial: string | null;
  purchase_date: string;
  invoice: string;
};

const profile = {
  inventory_name: "Household equipment inventory",
  requested_fields: [
    "item",
    "category",
    "manufacturer",
    "model",
    "serial",
    "purchase_date",
    "source_locators",
  ],
  source_formats: ["pdf", "image", "email", "spreadsheet"],
  merge_rule:
    "Merge records only when the same serial is present, or when invoice, manufacturer, model, and purchase date all match. Do not merge name-only matches.",
  review_requirements: [
    "Export one row per distinct item.",
    "Keep every source locator used for a row.",
    "Flag every unreadable serial instead of guessing it.",
  ],
};

const records: InventoryRecord[] = [
  {
    record_id: "record-001",
    format: "pdf",
    source_locator: "https://inventory.example.test/r001.pdf",
    item: "Kitchen refrigerator",
    manufacturer: "Alpine",
    model: "AR-400",
    serial: "ALP-400-001",
    purchase_date: "2025-01-12",
    invoice: "INV-1001",
  },
  {
    record_id: "record-002",
    format: "email",
    source_locator: "https://inventory.example.test/r002.eml",
    item: "Kitchen refrigerator",
    manufacturer: "Alpine",
    model: "AR-400",
    serial: "ALP-400-001",
    purchase_date: "2025-01-12",
    invoice: "INV-1001",
  },
  {
    record_id: "record-003",
    format: "image",
    source_locator: "https://inventory.example.test/r003.jpg",
    item: "Laundry washer",
    manufacturer: "Bright",
    model: "BW-220",
    serial: "BRT-220-002",
    purchase_date: "2025-02-03",
    invoice: "INV-1002",
  },
  {
    record_id: "record-004",
    format: "spreadsheet",
    source_locator: "https://inventory.example.test/r004.csv",
    item: "Laundry washer",
    manufacturer: "Bright",
    model: "BW-220",
    serial: "BRT-220-002",
    purchase_date: "2025-02-03",
    invoice: "INV-1002",
  },
  {
    record_id: "record-005",
    format: "pdf",
    source_locator: "https://inventory.example.test/r005.pdf",
    item: "Living room television",
    manufacturer: "Cobalt",
    model: "CT-55",
    serial: "COB-055-003",
    purchase_date: "2025-03-18",
    invoice: "INV-1003",
  },
  {
    record_id: "record-006",
    format: "email",
    source_locator: "https://inventory.example.test/r006.eml",
    item: "Living room television",
    manufacturer: "Cobalt",
    model: "CT-55",
    serial: "COB-055-003",
    purchase_date: "2025-03-18",
    invoice: "INV-1003",
  },
  {
    record_id: "record-007",
    format: "spreadsheet",
    source_locator: "https://inventory.example.test/r007.csv",
    item: "Bedroom air purifier",
    manufacturer: "Delta",
    model: "DP-80",
    serial: "DEL-080-004",
    purchase_date: "2025-04-22",
    invoice: "INV-1004",
  },
  {
    record_id: "record-008",
    format: "image",
    source_locator: "https://inventory.example.test/r008.jpg",
    item: "Bedroom air purifier",
    manufacturer: "Delta",
    model: "DP-80",
    serial: "DEL-080-004",
    purchase_date: "2025-04-22",
    invoice: "INV-1004",
  },
  {
    record_id: "record-009",
    format: "email",
    source_locator: "https://inventory.example.test/r009.eml",
    item: "Office monitor",
    manufacturer: "Echo",
    model: "EM-27",
    serial: "ECH-027-005",
    purchase_date: "2025-05-09",
    invoice: "INV-1005",
  },
  {
    record_id: "record-010",
    format: "pdf",
    source_locator: "https://inventory.example.test/r010.pdf",
    item: "Office monitor",
    manufacturer: "Echo",
    model: "EM-27",
    serial: "ECH-027-005",
    purchase_date: "2025-05-09",
    invoice: "INV-1005",
  },
  {
    record_id: "record-011",
    format: "image",
    source_locator: "https://inventory.example.test/r011.jpg",
    item: "Hallway vacuum",
    manufacturer: "Fjord",
    model: "FV-10",
    serial: "FJO-010-006",
    purchase_date: "2025-06-14",
    invoice: "INV-1006",
  },
  {
    record_id: "record-012",
    format: "spreadsheet",
    source_locator: "https://inventory.example.test/r012.csv",
    item: "Patio grill",
    manufacturer: "Granite",
    model: "GG-3",
    serial: "GRA-003-007",
    purchase_date: "2025-07-01",
    invoice: "INV-1007",
  },
  {
    record_id: "record-013",
    format: "pdf",
    source_locator: "https://inventory.example.test/r013.pdf",
    item: "Garage freezer",
    manufacturer: "Harbor",
    model: "HF-200",
    serial: "HBR-200-008",
    purchase_date: "2025-07-19",
    invoice: "INV-1008",
  },
  {
    record_id: "record-014",
    format: "email",
    source_locator: "https://inventory.example.test/r014.eml",
    item: "Guest-room fan",
    manufacturer: "Indigo",
    model: "IF-12",
    serial: null,
    purchase_date: "2025-08-02",
    invoice: "INV-1009",
  },
  {
    record_id: "record-015",
    format: "image",
    source_locator: "https://inventory.example.test/r015.jpg",
    item: "Entry lamp",
    manufacturer: "Juniper",
    model: "JL-4",
    serial: "JUN-004-010",
    purchase_date: "2025-08-28",
    invoice: "INV-1010",
  },
  {
    record_id: "record-016",
    format: "spreadsheet",
    source_locator: "https://inventory.example.test/r016.csv",
    item: "Dining microwave",
    manufacturer: "Kite",
    model: "KM-900",
    serial: "KIT-900-011",
    purchase_date: "2025-09-11",
    invoice: "INV-1011",
  },
  {
    record_id: "record-017",
    format: "pdf",
    source_locator: "https://inventory.example.test/r017.pdf",
    item: "Workshop sander",
    manufacturer: "Lumen",
    model: "LS-7",
    serial: "LUM-007-012",
    purchase_date: "2025-10-06",
    invoice: "INV-1012",
  },
  {
    record_id: "record-018",
    format: "email",
    source_locator: "https://inventory.example.test/r018.eml",
    item: "Child-room desk lamp",
    manufacturer: "Mica",
    model: "MD-2",
    serial: null,
    purchase_date: "2025-10-21",
    invoice: "INV-1013",
  },
  {
    record_id: "record-019",
    format: "image",
    source_locator: "https://inventory.example.test/r019.jpg",
    item: "Basement dehumidifier",
    manufacturer: "North",
    model: "ND-45",
    serial: "NOR-045-014",
    purchase_date: "2025-11-03",
    invoice: "INV-1014",
  },
  {
    record_id: "record-020",
    format: "spreadsheet",
    source_locator: "https://inventory.example.test/r020.csv",
    item: "Roof snow blower",
    manufacturer: "Orbit",
    model: "OS-8",
    serial: "ORB-008-015",
    purchase_date: "2025-11-25",
    invoice: "INV-1015",
  },
];

const docs = `# Synthetic mixed-record inventory API

Fixture version: ${fixtureVersion}

This deterministic service contains a synthetic inventory profile and twenty
source records. The records use PDF, image, email, and spreadsheet formats.
It has no connection to a retailer, manufacturer, mailbox, or real account.
Every record is test data. The base URL is the URL that served this document.

## Operations

- \`GET /v1/profile\`: return requested inventory fields, source formats, the
  evidence-based merge rule, and review requirements.
- \`GET /v1/records\`: return all twenty records. Each record has a format,
  stable source locator, item details, serial (null means unreadable), purchase
  date, and invoice number.

There are fifteen distinct items. Five pairs are duplicates because each pair
has the same serial, invoice, manufacturer, model, and purchase date. Records
with a null serial are unreadable and must be flagged. Do not merge records
using an item name alone. The service is read-only and has no write operation.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, {
    status,
    headers: { "cache-control": "no-store" },
  });
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "inventory", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, {
        headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" },
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json(profile);
    }
    if (request.method === "GET" && url.pathname === "/v1/records") {
      return json({ records, next_page_token: null });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic mixed-record inventory API listening on http://127.0.0.1:${server.port}`);
