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
    <section className="onboarding-shell" aria-label="Noema onboarding">
      <div className="onboarding-panel">
        <p className="eyebrow">First run</p>
        <h1>Connect {providerName}</h1>
        <p>
          Noema needs an authenticated provider account before it can open your
          local chat.
        </p>

        {providerStatus ? <ProviderStatus status={providerStatus} /> : null}

        {canStart ? (
          <button type="button" onClick={onConnect}>
            Connect {providerName}
          </button>
        ) : null}

        {waiting ? <AuthAttempt attempt={attempt} onCheck={onCheck} /> : null}

        {complete ? <p className="onboarding-success">Provider connected. Starting chat.</p> : null}

        {failed ? (
          <div className="onboarding-retry">
            <p>{attempt?.errorMessage ?? statusCopy[attempt?.status ?? "FAILED"]}</p>
            <button type="button" onClick={onRetry}>
              Try again
            </button>
          </div>
        ) : null}

        {error ? <p className="onboarding-error">{error}</p> : null}
      </div>
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
    <div className="auth-attempt" aria-live="polite">
      <strong>{attempt.status === "STARTING" ? "Starting login" : "Waiting for login"}</strong>
      {attempt.verificationUrl ? (
        <a href={attempt.verificationUrl} target="_blank" rel="noreferrer">
          Open login page
        </a>
      ) : null}
      {attempt.userCode ? <code>{attempt.userCode}</code> : null}
      {attempt.instructions ? <p>{attempt.instructions}</p> : null}
      <button type="button" onClick={onCheck}>
        {attempt.status === "WAITING_FOR_USER" ? "Continue" : "Check status"}
      </button>
    </div>
  );
}

function ProviderStatus({ status }: { status: ProviderAccountStatus }) {
  return <p className="onboarding-status">Provider status: {statusCopy[status]}</p>;
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
