type CeremonyStart<T> = {
  ceremonyId: string;
  options: { publicKey: T };
};

export type RegisteredPasskey = {
  credentialId: string;
  current: boolean;
};

export class PasskeyRequestError extends Error {
  constructor(readonly status: number) {
    super("Noema could not complete the passkey request.");
  }
}

export function passkeysSupported() {
  return Boolean(window.PublicKeyCredential && navigator.credentials);
}

export async function enrollPasskey() {
  const start = await postJson<CeremonyStart<PublicKeyCredentialCreationOptionsJSON>>(
    "/auth/passkey/register/start"
  );
  const credential = await navigator.credentials.create({
    publicKey: creationOptionsFromJson(start.options.publicKey)
  });
  if (!(credential instanceof PublicKeyCredential)) {
    throw new Error("Passkey creation was not completed.");
  }
  await postJson("/auth/passkey/register/finish", {
    ceremonyId: start.ceremonyId,
    credential: credentialToJson(credential)
  });
}

export async function authenticateWithPasskey() {
  const start = await postJson<CeremonyStart<PublicKeyCredentialRequestOptionsJSON>>(
    "/auth/passkey/login/start"
  );
  const credential = await navigator.credentials.get({
    publicKey: requestOptionsFromJson(start.options.publicKey)
  });
  if (!(credential instanceof PublicKeyCredential)) {
    throw new Error("Passkey authentication was not completed.");
  }
  await postJson("/auth/passkey/login/finish", {
    ceremonyId: start.ceremonyId,
    credential: credentialToJson(credential)
  });
}

export class RecoveryRequestError extends Error {
  constructor(readonly status: number) {
    super("Noema could not authorize recovery.");
  }
}

export async function authorizeRecovery(code: string) {
  const response = await fetch("/auth/recovery", {
    method: "POST",
    credentials: "same-origin",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ code })
  });
  if (!response.ok) throw new RecoveryRequestError(response.status);
}

export async function registeredPasskeys(): Promise<RegisteredPasskey[]> {
  const response = await fetch("/auth/passkeys", {
    credentials: "same-origin",
    cache: "no-store"
  });
  if (!response.ok) throw new PasskeyRequestError(response.status);
  return response.json() as Promise<RegisteredPasskey[]>;
}

export async function removePasskey(credentialId: string) {
  await postJson("/auth/passkey/remove", { credentialId });
}

export async function logoutBrowserSessions(all: boolean) {
  await postJson(all ? "/auth/logout/all" : "/auth/logout");
}

function creationOptionsFromJson(
  options: PublicKeyCredentialCreationOptionsJSON
): PublicKeyCredentialCreationOptions {
  if (typeof PublicKeyCredential.parseCreationOptionsFromJSON === "function") {
    return PublicKeyCredential.parseCreationOptionsFromJSON(options);
  }
  return {
    ...options,
    challenge: decodeBase64Url(options.challenge),
    user: { ...options.user, id: decodeBase64Url(options.user.id) },
    excludeCredentials: options.excludeCredentials?.map((credential) => ({
      ...credential,
      type: "public-key" as const,
      id: decodeBase64Url(credential.id),
      transports: credential.transports as AuthenticatorTransport[] | undefined
    }))
  } as unknown as PublicKeyCredentialCreationOptions;
}

function requestOptionsFromJson(
  options: PublicKeyCredentialRequestOptionsJSON
): PublicKeyCredentialRequestOptions {
  if (typeof PublicKeyCredential.parseRequestOptionsFromJSON === "function") {
    return PublicKeyCredential.parseRequestOptionsFromJSON(options);
  }
  return {
    ...options,
    challenge: decodeBase64Url(options.challenge),
    allowCredentials: options.allowCredentials?.map((credential) => ({
      ...credential,
      type: "public-key" as const,
      id: decodeBase64Url(credential.id),
      transports: credential.transports as AuthenticatorTransport[] | undefined
    }))
  } as unknown as PublicKeyCredentialRequestOptions;
}

function credentialToJson(credential: PublicKeyCredential): PublicKeyCredentialJSON | object {
  if (typeof credential.toJSON === "function") return credential.toJSON();

  const common = {
    id: credential.id,
    rawId: encodeBase64Url(credential.rawId),
    type: credential.type,
    clientExtensionResults: credential.getClientExtensionResults()
  };
  if (credential.response instanceof AuthenticatorAttestationResponse) {
    return {
      ...common,
      response: {
        attestationObject: encodeBase64Url(credential.response.attestationObject),
        clientDataJSON: encodeBase64Url(credential.response.clientDataJSON),
        transports: credential.response.getTransports?.()
      }
    };
  }
  const response = credential.response as AuthenticatorAssertionResponse;
  return {
    ...common,
    response: {
      authenticatorData: encodeBase64Url(response.authenticatorData),
      clientDataJSON: encodeBase64Url(response.clientDataJSON),
      signature: encodeBase64Url(response.signature),
      userHandle: response.userHandle ? encodeBase64Url(response.userHandle) : null
    }
  };
}

function decodeBase64Url(value: string): ArrayBuffer {
  const normalized = value.replaceAll("-", "+").replaceAll("_", "/");
  const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");
  const binary = atob(padded);
  return Uint8Array.from(binary, (character) => character.charCodeAt(0)).buffer;
}

function encodeBase64Url(value: ArrayBuffer): string {
  const bytes = new Uint8Array(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

async function postJson<T>(path: string, body?: unknown): Promise<T> {
  const response = await fetch(path, {
    method: "POST",
    credentials: "same-origin",
    headers: body === undefined ? undefined : { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body)
  });
  if (!response.ok) {
    throw new PasskeyRequestError(response.status);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}
