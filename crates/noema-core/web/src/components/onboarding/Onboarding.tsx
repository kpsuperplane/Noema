import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import * as stylex from "@stylexjs/stylex";
import { ErrorMarker } from "../ErrorMarker";
import { AuthAttempt } from "./AuthAttempt";
import { ProviderStatus } from "./ProviderStatus";
import { statusCopy } from "./statusCopy";
import type { OnboardingStatus, ProviderAuthAttemptView } from "./types";

export const PROVIDER_AUTH_POLL_INTERVAL_MS = 5_000;

export function Onboarding({
  onboarding,
  attempt,
  error,
  onConnect,
  onRetry
}: {
  onboarding: OnboardingStatus;
  attempt: ProviderAuthAttemptView | null;
  error: string | null;
  onConnect: () => void;
  onRetry: () => void;
}) {
  const step = onboarding.steps.find((candidate) => candidate.id === "connect_provider_account");
  const providerName = step?.displayName ?? "provider";
  const providerStatus = step?.providerAccountStatus;
  const waiting = attempt?.status === "STARTING" || attempt?.status === "WAITING_FOR_USER";
  const complete = attempt?.status === "COMPLETED";
  const failed = attempt ? isRetryableTerminalStatus(attempt.status) : false;
  const canStart = !attempt && !complete;

  return (
    <section {...stylex.props(styles.root)} aria-label="Noema onboarding">
      <Card {...stylex.props(styles.card)} padding={4}>
        <p {...stylex.props(styles.eyebrow)}>First run</p>
        <h1 {...stylex.props(styles.title)}>Connect {providerName}</h1>
        <p {...stylex.props(styles.description)}>
          Noema needs an authenticated provider account before it can open your local chat.
        </p>

        {providerStatus ? <ProviderStatus status={providerStatus} /> : null}

        {canStart ? (
          <Button type="button" label={`Connect ${providerName}`} onClick={onConnect}>
            Connect {providerName}
          </Button>
        ) : null}

        {waiting ? <AuthAttempt attempt={attempt} /> : null}

        {complete ? <p {...stylex.props(styles.connected)}>Provider connected. Starting chat.</p> : null}

        {failed ? (
          <div {...stylex.props(styles.retry)}>
            <p>{attempt?.errorMessage ?? statusCopy[attempt?.status ?? "FAILED"]}</p>
            <Button type="button" label="Try again" onClick={onRetry}>
              Try again
            </Button>
          </div>
        ) : null}

        {error ? <ErrorMarker message={error} /> : null}
      </Card>
    </section>
  );
}

const styles = stylex.create({
  root: {
    display: "grid",
    width: "min(760px, 100%)",
    minHeight: "100%",
    alignContent: "center",
    marginInline: "auto",
    padding: "34px 24px",
    "@media (max-width: 760px)": {
      alignContent: "start",
      padding: "28px 20px"
    }
  },
  card: {
    display: "grid",
    minWidth: 0,
    gap: 14,
    paddingTop: 18,
    paddingBottom: 18
  },
  eyebrow: {
    margin: 0,
    fontFamily: "var(--font-mono)",
    fontSize: 11,
    letterSpacing: "0.12em",
    color: "var(--text-accent)",
    textTransform: "uppercase"
  },
  title: {
    margin: 0,
    fontFamily: "var(--font-heading)",
    fontSize: 34,
    lineHeight: 1.1,
    letterSpacing: 0,
    color: "var(--foreground)",
    overflowWrap: "anywhere",
    "@media (max-width: 760px)": {
      fontSize: 30
    }
  },
  description: {
    margin: 0,
    maxWidth: 560,
    color: "var(--muted-foreground)",
    overflowWrap: "anywhere"
  },
  connected: {
    margin: 0,
    color: "var(--pine-700)"
  },
  retry: {
    display: "grid",
    gap: 10
  }
});

function isRetryableTerminalStatus(status: ProviderAuthAttemptView["status"]) {
  return status === "FAILED" || status === "EXPIRED" || status === "CANCELLED";
}

export function isProviderAuthAttemptPending(status: ProviderAuthAttemptView["status"]) {
  return status === "STARTING" || status === "WAITING_FOR_USER";
}
