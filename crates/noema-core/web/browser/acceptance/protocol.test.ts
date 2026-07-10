import { describe, expect, test } from "bun:test";

import {
  MAX_READY_FRAME_BYTES,
  MAX_READY_OUTPUT_BYTES,
  ReadinessParser,
  type BrowserReady
} from "./protocol";

const validReady: BrowserReady = Object.freeze({
  schemaVersion: 1,
  kind: "ready",
  scenario: "boot",
  origin: "http://127.0.0.1:43123",
  startUrl: "http://127.0.0.1:43123/"
});

function frame(value: unknown = validReady): string {
  return `NOEMA_BROWSER_READY ${JSON.stringify(value)}\n`;
}

describe("ReadinessParser", () => {
  test("accepts one strict boot readiness frame amid bounded libtest noise", () => {
    const parser = new ReadinessParser();
    parser.push("running 1 test\n");
    parser.push(frame());
    parser.push("test daemon::browser_acceptance::fixture_server ... ok\n");
    expect(parser.finish()).toEqual(validReady);
  });

  test("assembles readiness across arbitrary chunk boundaries", () => {
    const parser = new ReadinessParser();
    const input = frame();
    for (const character of input) parser.push(character);
    expect(parser.finish()).toEqual(validReady);
  });

  test("rejects duplicate readiness frames", () => {
    const parser = new ReadinessParser();
    parser.push(frame());
    expect(() => parser.push(frame())).toThrow("duplicate browser readiness frame");
  });

  test("rejects a repeated frame after unrelated output", () => {
    const parser = new ReadinessParser();
    parser.push(frame());
    parser.push("noise\n");
    expect(() => parser.push(frame())).toThrow("duplicate browser readiness frame");
  });

  test("rejects overlong lines and total output", () => {
    const lineParser = new ReadinessParser();
    expect(() => lineParser.push("x".repeat(MAX_READY_FRAME_BYTES + 1))).toThrow(
      "browser fixture output line exceeded limit"
    );

    const totalParser = new ReadinessParser();
    const line = `${"x".repeat(100)}\n`;
    expect(() => {
      while (true) totalParser.push(line);
    }).toThrow("browser fixture output exceeded limit");
    expect(MAX_READY_OUTPUT_BYTES).toBeGreaterThan(MAX_READY_FRAME_BYTES);
  });

  test.each([
    ["malformed JSON", "{", "invalid browser readiness frame"],
    ["missing key", { ...validReady, startUrl: undefined }, "invalid browser readiness frame"],
    ["extra key", { ...validReady, token: "x" }, "invalid browser readiness frame"],
    ["wrong version", { ...validReady, schemaVersion: 2 }, "invalid browser readiness frame"],
    ["wrong kind", { ...validReady, kind: "started" }, "invalid browser readiness frame"],
    ["wrong scenario", { ...validReady, scenario: "chat" }, "invalid browser readiness frame"]
  ])("rejects %s", (_name, value, message) => {
    const parser = new ReadinessParser();
    const input = typeof value === "string" ? `NOEMA_BROWSER_READY ${value}\n` : frame(value);
    expect(() => parser.push(input)).toThrow(message as string);
  });

  test.each([
    ["external host", "http://example.test:43123", "http://example.test:43123/"],
    ["credentials", "http://user:pass@127.0.0.1:43123", "http://user:pass@127.0.0.1:43123/"],
    ["query", "http://127.0.0.1:43123", "http://127.0.0.1:43123/?secret=x"],
    ["fragment", "http://127.0.0.1:43123", "http://127.0.0.1:43123/#secret"],
    ["non-root path", "http://127.0.0.1:43123", "http://127.0.0.1:43123/setup"],
    ["mismatched origin", "http://127.0.0.1:43123", "http://127.0.0.1:43124/"],
    ["zero port", "http://127.0.0.1:0", "http://127.0.0.1:0/"],
    ["implicit port", "http://127.0.0.1", "http://127.0.0.1/"],
    ["HTTPS", "https://127.0.0.1:43123", "https://127.0.0.1:43123/"]
  ])("rejects %s URLs", (_name, origin, startUrl) => {
    const parser = new ReadinessParser();
    expect(() => parser.push(frame({ ...validReady, origin, startUrl }))).toThrow(
      "invalid browser readiness frame"
    );
  });

  test("rejects finishing without readiness and truncated readiness", () => {
    expect(() => new ReadinessParser().finish()).toThrow("browser readiness frame missing");
    const parser = new ReadinessParser();
    parser.push(frame().slice(0, -1));
    expect(() => parser.finish()).toThrow("browser readiness frame missing");
  });
});
