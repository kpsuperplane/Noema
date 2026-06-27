import type {
  OnboardingStatus,
  ProviderAccountStatus,
  ProviderAuthAttemptView
} from "../generated/noema";

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
  const providerName = step?.display_name ?? "provider";
  const providerStatus = step?.provider_account_status;
  const waiting = attempt?.status === "starting" || attempt?.status === "waiting_for_user";
  const complete = attempt?.status === "completed";
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
            <p>{attempt?.error_message ?? statusCopy[attempt?.status ?? "failed"]}</p>
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
      <strong>{attempt.status === "starting" ? "Starting login" : "Waiting for login"}</strong>
      {attempt.verification_url ? (
        <a href={attempt.verification_url} target="_blank" rel="noreferrer">
          Open login page
        </a>
      ) : null}
      {attempt.user_code ? <code>{attempt.user_code}</code> : null}
      {attempt.instructions ? <p>{attempt.instructions}</p> : null}
      <button type="button" onClick={onCheck}>
        {attempt.status === "waiting_for_user" ? "Continue" : "Check status"}
      </button>
    </div>
  );
}

function ProviderStatus({ status }: { status: ProviderAccountStatus }) {
  return <p className="onboarding-status">Provider status: {statusCopy[status]}</p>;
}

function isRetryableTerminalStatus(status: ProviderAuthAttemptView["status"]) {
  return status === "failed" || status === "expired" || status === "cancelled";
}

const statusCopy: Record<ProviderAccountStatus | ProviderAuthAttemptView["status"], string> = {
  unknown: "Not checked yet",
  checking: "Checking",
  authenticated: "Connected",
  unauthenticated: "Not connected",
  unavailable: "Unavailable",
  starting: "Starting login",
  waiting_for_user: "Waiting for login",
  completed: "Connected",
  failed: "Login failed",
  expired: "Login expired",
  cancelled: "Login cancelled"
};
