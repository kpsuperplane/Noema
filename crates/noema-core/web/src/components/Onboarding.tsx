import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import type {
  GraphqlProviderAuthAttemptStatus,
  OnboardingStatusQuery,
  ProviderAuthAttemptQuery,
  StartProviderAuthAttemptMutation
} from "../generated/graphql";

type OnboardingStatus = OnboardingStatusQuery["onboardingStatus"];
type ProviderAuthAttemptView =
  | StartProviderAuthAttemptMutation["startProviderAuthAttempt"]
  | NonNullable<ProviderAuthAttemptQuery["providerAuthAttempt"]>;
type ProviderAccountStatus = NonNullable<OnboardingStatus["steps"][number]["providerAccountStatus"]>;

export function Onboarding({
  onboarding,
  attempt,
  error,
  onConnect,
  onCheck,
  onRetry
}: {
  onboarding: OnboardingStatus;
  attempt: ProviderAuthAttemptView | null;
  error: string | null;
  onConnect: () => void;
  onCheck: () => void;
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
      className="mx-auto grid min-h-[calc(100vh-68px)] w-[min(760px,100%)] content-center px-6 py-[34px] max-[760px]:min-h-[calc(100vh-118px)] max-[760px]:content-start max-[760px]:px-5 max-[760px]:py-7"
      aria-label="Noema onboarding"
    >
      <Card className="grid min-w-0 gap-3.5 py-[18px]">
        <CardContent className="grid gap-3.5">
          <p className="m-0 font-mono text-[11px] tracking-[0.12em] text-[var(--text-accent)] uppercase">First run</p>
          <h1 className="m-0 font-heading text-[34px] leading-[1.1] tracking-normal text-foreground [overflow-wrap:anywhere] max-[760px]:text-3xl">
            Connect {providerName}
          </h1>
          <p className="m-0 max-w-[560px] text-muted-foreground [overflow-wrap:anywhere]">
            Noema needs an authenticated provider account before it can open your
            local chat.
          </p>

          {providerStatus ? <ProviderStatus status={providerStatus} /> : null}

          {canStart ? (
            <Button type="button" onClick={onConnect}>
              Connect {providerName}
            </Button>
          ) : null}

          {waiting ? <AuthAttempt attempt={attempt} onCheck={onCheck} /> : null}

          {complete ? <p className="m-0 text-[var(--pine-700)]">Provider connected. Starting chat.</p> : null}

          {failed ? (
            <div className="grid gap-2.5">
              <p>{attempt?.errorMessage ?? statusCopy[attempt?.status ?? "FAILED"]}</p>
              <Button type="button" onClick={onRetry}>
                Try again
              </Button>
            </div>
          ) : null}

          {error ? <p className="m-0 text-[var(--red-700)]">{error}</p> : null}
        </CardContent>
      </Card>
    </section>
  );
}

function AuthAttempt({
  attempt,
  onCheck
}: {
  attempt: ProviderAuthAttemptView;
  onCheck: () => void;
}) {
  return (
    <Card className="grid min-w-0 max-w-[560px] gap-2.5" aria-live="polite">
      <CardContent className="grid gap-3.5">
        <strong>{attempt.status === "STARTING" ? "Starting login" : "Waiting for login"}</strong>
        {attempt.verificationUrl ? (
          <Button render={<a href={attempt.verificationUrl} target="_blank" rel="noreferrer" />}>
            Open login page
          </Button>
        ) : null}
        {attempt.userCode ? (
          <code className="w-fit max-w-full rounded-md bg-[var(--surface-sunken)] px-2 py-1.5 font-mono text-lg leading-[1.35] whitespace-normal [overflow-wrap:anywhere]">
            {attempt.userCode}
          </code>
        ) : null}
        {attempt.instructions ? <p>{attempt.instructions}</p> : null}
        <Button type="button" onClick={onCheck}>
          {attempt.status === "WAITING_FOR_USER" ? "Continue" : "Check status"}
        </Button>
      </CardContent>
    </Card>
  );
}

function ProviderStatus({ status }: { status: ProviderAccountStatus }) {
  return (
    <Badge variant="outline" className="font-mono text-xs text-muted-foreground">
      Provider status: {statusCopy[status]}
    </Badge>
  );
}

function isRetryableTerminalStatus(status: ProviderAuthAttemptView["status"]) {
  return status === "FAILED" || status === "EXPIRED" || status === "CANCELLED";
}

const statusCopy: Record<ProviderAccountStatus | GraphqlProviderAuthAttemptStatus, string> = {
  UNKNOWN: "Not checked yet",
  CHECKING: "Checking",
  AUTHENTICATED: "Connected",
  UNAUTHENTICATED: "Not connected",
  UNAVAILABLE: "Unavailable",
  STARTING: "Starting login",
  WAITING_FOR_USER: "Waiting for login",
  COMPLETED: "Connected",
  FAILED: "Login failed",
  EXPIRED: "Login expired",
  CANCELLED: "Login cancelled"
};
