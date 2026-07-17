type ClientIdCrypto = {
  randomUUID?: () => string;
  getRandomValues?: <T extends Uint8Array>(array: T) => T;
};

type ClientIdCryptoWithRandomValues = ClientIdCrypto & {
  getRandomValues: <T extends Uint8Array>(array: T) => T;
};

export function createClientId(source: ClientIdCrypto = globalThis.crypto): string {
  if (typeof source.randomUUID === "function") {
    return source.randomUUID();
  }

  if (hasRandomValues(source)) {
    return uuidFromRandomBytes(source);
  }

  return `fallback-${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

function hasRandomValues(source: ClientIdCrypto): source is ClientIdCryptoWithRandomValues {
  return typeof source.getRandomValues === "function";
}

function uuidFromRandomBytes(source: ClientIdCryptoWithRandomValues): string {
  const bytes = source.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));

  return [
    hex.slice(0, 4).join(""),
    hex.slice(4, 6).join(""),
    hex.slice(6, 8).join(""),
    hex.slice(8, 10).join(""),
    hex.slice(10, 16).join("")
  ].join("-");
}
