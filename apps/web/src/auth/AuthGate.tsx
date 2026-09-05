import React from "react";
import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/Stack";
import { TextInput } from "@astryxdesign/core/TextInput";
import { AppBootSkeleton } from "@/components/shell/AppBootSkeleton";
import {
  SetupFrame,
  SetupCard,
  SetupActions,
  SetupNote
} from "@/components/shell/SetupFrame";
import { ErrorMarker } from "@/components/ErrorMarker";
import { isTauriRuntime } from "@/graphql/transportMode";
import { pwaRuntime } from "@/pwa/runtime";
import { hasAuthenticatedSentinel } from "@/pwa/storage";
import {
  authenticateWithPasskey,
  authorizeRecovery,
  enrollPasskey,
  passkeysSupported,
  RecoveryRequestError,
  PasskeyRequestError
} from "./passkey";

type AuthState =
  | "loading"
  | "authenticated"
  | "setup_ready"
  | "login_required"
  | "recovery_required"
  | "recovery_enroll"
  | "recovery_expired"
  | "recovery_complete"
  | "unavailable";
type AuthStatus = { state: "authenticated" | "setup_ready" | "login_required" };

export function AuthGate({ children }: { children: React.ReactNode }) {
  const desktop = isTauriRuntime();
  const nativeAuthorization =
    !desktop &&
    new URLSearchParams(window.location.search).get("native_authorization") ===
      "resume";
  const pwa = React.useSyncExternalStore(
    pwaRuntime.subscribe,
    pwaRuntime.getSnapshot,
    pwaRuntime.getSnapshot
  );
  const [state, setState] = React.useState<AuthState>(
    desktop ? "authenticated" : "loading"
  );
  const [working, setWorking] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [recoveryCode, setRecoveryCode] = React.useState("");
  const request = React.useRef<AbortController | null>(null);
  const primary = React.useRef<HTMLButtonElement>(null);
  const recoveryForm = React.useRef<HTMLFormElement>(null);
  const initializeAuthentication =
    React.useCallback(async (): Promise<AuthState> => {
      const next = await readAuthStatus();
      if (next === "authenticated") {
        await pwaRuntime.authenticated();
      } else if (next === "unavailable" && (await hasAuthenticatedSentinel())) {
        pwaRuntime.goOffline();
        return "authenticated";
      }
      return next;
    }, []);

  React.useEffect(() => {
    if (desktop) return;
    let active = true;
    void initializeAuthentication().then((next) => {
      if (active) setState(next);
    });
    return () => {
      active = false;
      request.current?.abort();
    };
  }, [desktop, initializeAuthentication]);

  // Reauthentication must not hide an active recovery flow.
  const visibleState =
    state === "authenticated" &&
    (nativeAuthorization || pwa.state === "auth_required")
      ? "login_required"
      : state;
  React.useEffect(() => {
    if (visibleState !== "recovery_enroll") return;
    const expiry = window.setTimeout(
      () => {
        request.current?.abort();
        setError(null);
        setState("recovery_expired");
      },
      5 * 60 * 1000
    );
    return () => window.clearTimeout(expiry);
  }, [visibleState]);

  React.useEffect(() => {
    function restoreAccessStep() {
      if (
        state === "authenticated" &&
        !nativeAuthorization &&
        pwa.state !== "auth_required"
      )
        return;
      request.current?.abort();
      setRecoveryCode("");
      setError(null);
      const recovery = window.history.state?.noemaAccess === "recovery";
      setState(recovery ? "recovery_required" : "login_required");
      requestAnimationFrame(() => {
        if (recovery) recoveryForm.current?.querySelector("input")?.focus();
        else primary.current?.focus();
      });
    }
    window.addEventListener("popstate", restoreAccessStep);
    return () => window.removeEventListener("popstate", restoreAccessStep);
  }, [state, nativeAuthorization, pwa.state]);

  function showRecovery() {
    if (window.history.state?.noemaAccess !== "recovery") {
      window.history.pushState(
        { ...window.history.state, noemaAccess: "recovery" },
        ""
      );
    }
    setError(null);
    setState("recovery_required");
  }

  function finish() {
    if (nativeAuthorization) window.location.replace("/oauth/authorize");
    else setState("authenticated");
  }

  async function perform(recovery: boolean, authorize = false) {
    if (working) return;
    setWorking(true);
    setError(null);
    const controller = new AbortController();
    request.current = controller;
    pwaRuntime.setCriticalOperation("passkey", true);
    try {
      if (authorize) {
        await authorizeRecovery(recoveryCode.trim());
        setRecoveryCode("");
        setState("recovery_enroll");
      }
      if (recovery || visibleState === "setup_ready")
        await enrollPasskey(controller.signal);
      else await authenticateWithPasskey(controller.signal);
      await pwaRuntime.authenticated();
      if (recovery) setState("recovery_complete");
      else finish();
    } catch (caught) {
      setRecoveryCode("");
      if (controller.signal.aborted) return;
      if (caught instanceof RecoveryRequestError) {
        setError(
          caught.status === 401
            ? "That code was not accepted. Each attempt replaces it. Get the new code and try again."
            : "Noema could not check the code. Get the current code before you try again."
        );
      } else if (
        recovery &&
        caught instanceof PasskeyRequestError &&
        (caught.status === 401 || caught.status === 403)
      ) {
        setState("recovery_expired");
      } else {
        setError(
          caught instanceof DOMException && caught.name === "NotAllowedError"
            ? "The passkey prompt closed. You can try again."
            : "Noema could not complete the passkey request. Try again."
        );
      }
    } finally {
      request.current = null;
      pwaRuntime.setCriticalOperation("passkey", false);
      setWorking(false);
    }
  }

  function showLogin() {
    if (window.history.state?.noemaAccess === "recovery") {
      window.history.back();
      return;
    }
    setRecoveryCode("");
    setError(null);
    setState("login_required");
    requestAnimationFrame(() => primary.current?.focus());
  }
  function retryStatus() {
    setError(null);
    setState("loading");
    void initializeAuthentication().then(setState);
  }
  if (visibleState === "loading") return <AppBootSkeleton />;
  if (visibleState === "authenticated") return children;
  const supported = passkeysSupported();
  const unavailable = visibleState === "unavailable";
  const recovery = visibleState === "recovery_required";
  const enroll = visibleState === "recovery_enroll";
  const complete = visibleState === "recovery_complete";
  const expired = visibleState === "recovery_expired";
  const create = visibleState === "setup_ready";
  const title = unavailable
    ? "Noema is out of reach"
    : !supported
      ? "Try a passkey-ready browser"
      : working
        ? "Follow your device’s prompt"
        : complete
          ? "Your access is restored"
          : expired
            ? "Recovery time ran out"
            : enroll
              ? "Create your new passkey"
              : recovery
                ? "Recover access"
                : create
                  ? "Make Noema yours"
                  : "Welcome back";
  const intro = unavailable
    ? "Check your connection and try again."
    : !supported
      ? "Open Noema in a current browser."
      : working
        ? "Your device will guide you."
        : complete
          ? "Use your new passkey next time."
          : expired
            ? "Get a new code to continue."
            : enroll
              ? "Save it to your device or password manager."
              : recovery
                ? "Use a recovery code to add a passkey."
                : create
                  ? "Create a passkey to get started."
                  : "Your passkey opens the door.";
  return (
    <SetupFrame>
      <SetupCard title={title} intro={intro}>
        {error ? <ErrorMarker message={error} /> : null}
        {!supported && !unavailable ? (
          <SetupNote>
            Use Safari, Chrome, Edge, or Firefox with passkey support. Open the
            same Noema address there.
          </SetupNote>
        ) : null}
        {working ? (
          <SetupActions>
            <Button
              variant="secondary"
              label="Cancel"
              onClick={() => {
                request.current?.abort();
              }}
            />
          </SetupActions>
        ) : null}
        {supported && !working && recovery ? (
          <VStack
            as="form"
            ref={recoveryForm}
            gap={3}
            onSubmit={(event) => {
              event.preventDefault();
              void perform(true, true);
            }}
          >
            <TextInput
              type="password"
              label="Recovery code"
              value={recoveryCode}
              isRequired
              hasAutoFocus
              onChange={setRecoveryCode}
            />
            <details>
              <summary>Where is my recovery code?</summary>
              <p>
                Ask the person who runs your Noema server for the current code.
                Each attempt replaces it, even if the code is incorrect.
              </p>
              <p>
                If you run the server, read <code>web.recovery_code</code> in{" "}
                <code>config.yaml</code>.
              </p>
            </details>
            <SetupActions>
              <Button
                variant="secondary"
                label="Use passkey"
                onClick={showLogin}
              />
              <Button
                type="submit"
                variant="primary"
                label="Continue"
                isDisabled={!recoveryCode.trim()}
              />
            </SetupActions>
          </VStack>
        ) : null}
        {supported && !working && (create || enroll) ? (
          <SetupActions>
            <Button
              variant="primary"
              label="Create passkey"
              onClick={() => void perform(enroll)}
            />
          </SetupActions>
        ) : null}
        {supported && !working && visibleState === "login_required" ? (
          <SetupActions>
            <Button
              variant="secondary"
              label="Recover access"
              onClick={showRecovery}
            />
            <Button
              ref={primary}
              variant="primary"
              label="Use passkey"
              onClick={() => void perform(false)}
            />
          </SetupActions>
        ) : null}
        {complete ? (
          <>
            <SetupNote>
              Existing passkeys and connected apps still have access. Review
              them in Settings.
            </SetupNote>
            <SetupActions>
              <Button
                variant="primary"
                label="Continue to Noema"
                onClick={finish}
              />
            </SetupActions>
          </>
        ) : null}
        {expired ? (
          <SetupActions>
            <Button
              variant="secondary"
              label="Use passkey"
              onClick={showLogin}
            />
            <Button
              variant="primary"
              label="Enter a new code"
              onClick={showRecovery}
            />
          </SetupActions>
        ) : null}
        {unavailable ? (
          <SetupActions>
            <Button variant="primary" label="Try again" onClick={retryStatus} />
          </SetupActions>
        ) : null}
      </SetupCard>
    </SetupFrame>
  );
}

async function readAuthStatus(): Promise<AuthState> {
  try {
    const response = await fetch("/auth/status", {
      credentials: "same-origin"
    });
    if (!response.ok) return "unavailable";
    return ((await response.json()) as AuthStatus).state;
  } catch {
    return "unavailable";
  }
}
