import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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
    <section
      className="mx-auto grid min-h-full w-[min(760px,100%)] content-center px-6 py-[34px] max-[760px]:content-start max-[760px]:px-5 max-[760px]:py-7"
      aria-label="Noema onboarding"
    >
      <Card className="grid min-w-0 gap-3.5 py-[18px]">
        <CardContent className="grid gap-3.5">
          <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">First run</p>
          <h1 className="m-0 font-heading text-[34px] leading-[1.1] tracking-normal text-foreground [overflow-wrap:anywhere] max-[760px]:text-3xl">
            Connect {providerName}
          </h1>
          <p className="m-0 max-w-[560px] text-muted-foreground [overflow-wrap:anywhere]">
            Noema needs an authenticated provider account before it can open your local chat.
          </p>

          {providerStatus ? <ProviderStatus status={providerStatus} /> : null}

          {canStart ? (
            <Button type="button" onClick={onConnect}>
              Connect {providerName}
            </Button>
          ) : null}

          {waiting ? <AuthAttempt attempt={attempt} /> : null}

          {complete ? <p className="m-0 text-[var(--pine-700)]">Provider connected. Starting chat.</p> : null}

          {failed ? (
            <div className="grid gap-2.5">
              <p>{attempt?.errorMessage ?? statusCopy[attempt?.status ?? "FAILED"]}</p>
              <Button type="button" onClick={onRetry}>
                Try again
              </Button>
            </div>
          ) : null}

          {error ? <ErrorMarker message={error} /> : null}
        </CardContent>
      </Card>
    </section>
  );
}

function isRetryableTerminalStatus(status: ProviderAuthAttemptView["status"]) {
  return status === "FAILED" || status === "EXPIRED" || status === "CANCELLED";
}

export function isProviderAuthAttemptPending(status: ProviderAuthAttemptView["status"]) {
  return status === "STARTING" || status === "WAITING_FOR_USER";
}
