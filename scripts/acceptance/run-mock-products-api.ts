#!/usr/bin/env bun

// Synthetic product service for PA-031.
// It has no connection to a retailer, payment service, or shipping provider.

const port = Number(process.env.NOEMA_PRODUCTS_PORT ?? "3749");
const fixtureVersion = "2026-09-08-products-api-v1";

type Product = {
  id: string;
  name: string;
  price: number;
  shipping: number;
  tax: number;
  compatibility: { linux: boolean; memoryGb: number; usbCCharging: boolean };
  warrantyYears: number;
  deliveryDays: number;
  sourceUrl: string;
};

const profile = {
  budget: 600,
  currency: "USD",
  requiredCompatibility: {
    linux: true,
    minimumMemoryGb: 16,
    usbCCharging: true,
  },
  preferences: [
    "Stay within the total budget after shipping and tax.",
    "Reject products that fail any required compatibility condition.",
    "Prefer a longer warranty and delivery within five days when the total cost is feasible.",
  ],
};

const products: Product[] = [
  {
    id: "product-001",
    name: "CloudBook Air",
    price: 560,
    shipping: 25,
    tax: 45,
    compatibility: { linux: true, memoryGb: 16, usbCCharging: true },
    warrantyYears: 1,
    deliveryDays: 3,
    sourceUrl: "https://products.example.test/cloudbook-air",
  },
  {
    id: "product-002",
    name: "FieldBook",
    price: 575,
    shipping: 0,
    tax: 18,
    compatibility: { linux: true, memoryGb: 16, usbCCharging: true },
    warrantyYears: 2,
    deliveryDays: 5,
    sourceUrl: "https://products.example.test/fieldbook",
  },
  {
    id: "product-003",
    name: "StudioBook",
    price: 590,
    shipping: 0,
    tax: 10,
    compatibility: { linux: false, memoryGb: 32, usbCCharging: false },
    warrantyYears: 3,
    deliveryDays: 2,
    sourceUrl: "https://products.example.test/studiobook",
  },
];

const docs = `# Synthetic products API

Fixture version: ${fixtureVersion}

This deterministic service contains three synthetic laptop listings and one
preference profile. It has no connection to a retailer, payment service,
shipping provider, or real account. Every record is test data. The base URL is
the URL that served this document.

## Operations

- \`GET /v1/profile\`: return the total budget, currency, required Linux,
  memory, and USB-C charging constraints, plus preferences.
- \`GET /v1/products\`: return all three current listings. Each listing gives
  sticker price, shipping, tax, compatibility fields, warranty, delivery time,
  and a source URL. Total cost is price plus shipping plus tax.

The listings are read-only. CloudBook Air has the lowest sticker price among
the compatible products, but shipping and tax place its total above the $600
budget. FieldBook has a higher sticker price but remains within budget because
shipping and tax are lower. StudioBook has attractive warranty and delivery
values but fails the required Linux and USB-C compatibility checks. Do not buy
anything through this service.
`;

function json(value: unknown, status = 200) {
  return Response.json(value, { status, headers: { "cache-control": "no-store" } });
}

function productResponse(product: Product) {
  return {
    id: product.id,
    name: product.name,
    price: product.price,
    shipping: product.shipping,
    tax: product.tax,
    compatibility: {
      linux: product.compatibility.linux,
      memory_gb: product.compatibility.memoryGb,
      usb_c_charging: product.compatibility.usbCCharging,
    },
    warranty_years: product.warrantyYears,
    delivery_days: product.deliveryDays,
    source_url: product.sourceUrl,
  };
}

const server = Bun.serve({
  hostname: "0.0.0.0",
  port,
  fetch(request) {
    const url = new URL(request.url);
    if (request.method === "GET" && url.pathname === "/health") {
      return json({ ok: true, synthetic: true, service: "products", fixture: fixtureVersion });
    }
    if (request.method === "GET" && url.pathname === "/docs") {
      return new Response(docs, { headers: { "content-type": "text/markdown; charset=utf-8", "cache-control": "no-store" } });
    }
    if (request.method === "GET" && url.pathname === "/v1/profile") {
      return json({
        budget: profile.budget,
        currency: profile.currency,
        required_compatibility: profile.requiredCompatibility,
        preferences: profile.preferences,
      });
    }
    if (request.method === "GET" && url.pathname === "/v1/products") {
      return json({ products: products.map(productResponse), next_page_token: null });
    }
    return json({ error: "not_found" }, 404);
  },
});

console.log(`synthetic products API listening on http://127.0.0.1:${server.port}`);
