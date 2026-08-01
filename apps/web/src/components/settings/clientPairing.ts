const PAIRING_TTL_MS = 10 * 60 * 1000;

type PairingStartPayload = { pairingUri?: unknown };
export type ClientPairing = { pairingUri: string; expiresAt: number };

export async function startClientPairing(): Promise<ClientPairing> {
  const response = await fetch("/auth/client/pairing/start", {
    method: "POST", credentials: "same-origin", headers: { Accept: "application/json" }
  });
  if (!response.ok) {
    throw new Error(response.status === 501
      ? "Pairing requires a trusted HTTPS public origin that is not localhost."
      : "Client pairing could not be started. Check that this browser is signed in and try again.");
  }
  const payload = await response.json().catch(() => null) as PairingStartPayload | null;
  if (!payload || typeof payload.pairingUri !== "string" || !isPairingUri(payload.pairingUri)) {
    throw new Error("Client pairing returned an invalid response.");
  }
  return { pairingUri: payload.pairingUri, expiresAt: Date.now() + PAIRING_TTL_MS };
}

function isPairingUri(value: string): boolean {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "noema:" && parsed.hostname === "pair";
  } catch { return false; }
}
