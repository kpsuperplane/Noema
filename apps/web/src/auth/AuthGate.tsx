import React from "react";
import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/Stack";
import * as stylex from "@stylexjs/stylex";
import { AppBootSkeleton } from "@/components/shell/AppBootSkeleton";
import { SetupFrame } from "@/components/shell/SetupFrame";
import { isTauriRuntime } from "@/graphql/transportMode";
import {
  authenticateWithPasskey,
  enrollPasskey,
  passkeysSupported
} from "./passkey";

type AuthState =
  | "loading"
  | "authenticated"
  | "setup_required"
  | "setup_ready"
  | "login_required"
  | "unavailable";

type AuthStatus = { state: Exclude<AuthState, "loading" | "unavailable"> };

export function AuthGate({ children }: { children: React.ReactNode }) {
  const desktop = isTauriRuntime();
  const [state, setState] = React.useState<AuthState>(desktop ? "authenticated" : "loading");
  const [working, setWorking] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (desktop) return;
    let active = true;
    void readAuthStatus().then((next) => {
      if (active) setState(next);
    });
    return () => {
      active = false;
    };
  }, [desktop]);

  async function perform(action: () => Promise<void>) {
    setWorking(true);
    setError(null);
    try {
      await action();
      setState("authenticated");
    } catch (caught) {
      setError(
        caught instanceof DOMException && caught.name === "NotAllowedError"
          ? "The passkey prompt was cancelled or timed out."
          : "Noema could not verify that passkey. Try again."
      );
    } finally {
      setWorking(false);
    }
  }

  function retryStatus() {
    setError(null);
    setState("loading");
    void readAuthStatus().then(setState);
  }

  if (state === "loading") return <AppBootSkeleton />;
  if (state === "authenticated") return children;

  const supported = passkeysSupported();
  const setupReady = state === "setup_ready";
  const loginRequired = state === "login_required";

  return (
    <SetupFrame subtitle="Secure access">
      <VStack as="section" {...stylex.props(styles.root)}>
        <VStack gap={3} width="min(480px, 100%)" {...stylex.props(styles.content)}>
          <p {...stylex.props(styles.eyebrow)}>Private server</p>
          <h1 {...stylex.props(styles.title)}>
            {setupReady ? "Create your Noema passkey" : loginRequired ? "Unlock Noema" : "Secure Noema"}
          </h1>
          <p {...stylex.props(styles.description)}>
            {setupReady
              ? "Use your device or password manager to create the passkey for this Noema server."
              : loginRequired
                ? "Use the passkey registered to this server to continue."
                : state === "setup_required"
                  ? "Open the one-time setup link printed by the Noema server to register its first passkey."
                  : state === "unavailable"
                    ? "Noema could not read the server authentication state."
                    : "Checking server access…"}
          </p>

          {!supported && state !== "unavailable" ? (
            <p {...stylex.props(styles.error)}>
              This browser does not support the WebAuthn passkey APIs required by Noema.
            </p>
          ) : null}
          {error ? <p {...stylex.props(styles.error)}>{error}</p> : null}

          {setupReady && supported ? (
            <Button
              type="button"
              label="Create passkey"
              isDisabled={working}
              isLoading={working}
              onClick={() => void perform(enrollPasskey)}
            >
              Create passkey
            </Button>
          ) : null}
          {loginRequired && supported ? (
            <Button
              type="button"
              label="Continue with passkey"
              isDisabled={working}
              isLoading={working}
              onClick={() => void perform(authenticateWithPasskey)}
            >
              Continue with passkey
            </Button>
          ) : null}
          {state === "unavailable" ? (
            <Button type="button" variant="secondary" label="Try again" onClick={retryStatus}>
              Try again
            </Button>
          ) : null}
        </VStack>
      </VStack>
    </SetupFrame>
  );
}

async function readAuthStatus(): Promise<AuthState> {
  try {
    const response = await fetch("/auth/status", { credentials: "same-origin" });
    if (!response.ok) return "unavailable";
    return ((await response.json()) as AuthStatus).state;
  } catch {
    return "unavailable";
  }
}

const styles = stylex.create({
  root: {
    minHeight: "100%",
    justifyContent: "center",
    padding: "var(--spacing-6)",
    "@media (max-width: 640px)": {
      justifyContent: "flex-start",
      padding: "var(--spacing-4)"
    }
  },
  content: {
    marginInline: "auto"
  },
  eyebrow: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-mono)",
    fontSize: 11,
    letterSpacing: "0.12em",
    color: "var(--text-accent)",
    textTransform: "uppercase"
  },
  title: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 32,
    lineHeight: 1.1,
    color: "var(--foreground)"
  },
  description: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)"
  },
  error: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)"
  }
});
