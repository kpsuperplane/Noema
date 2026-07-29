import type React from "react";
import { Button } from "@astryxdesign/core/Button";
import { Card } from "@astryxdesign/core/Card";
import * as stylex from "@stylexjs/stylex";
import { openExternalUrlForAuth } from "@/graphql/externalUrls";
import { isTauriRuntime } from "@/graphql/transportMode";
import type { ProviderAuthAttemptView } from "./types";

export function AuthAttempt({
  attempt,
  onCancel
}: {
  attempt: ProviderAuthAttemptView;
  onCancel: () => void;
}) {
  async function openLoginPage(event: React.MouseEvent<HTMLButtonElement>) {
    if (!attempt.verificationUrl) {
      return;
    }

    if (isTauriRuntime()) {
      event.preventDefault();
    }

    const handled = await openExternalUrlForAuth(attempt.verificationUrl);
    if (!handled && event.defaultPrevented) {
      window.open(attempt.verificationUrl, "_blank", "noopener,noreferrer");
    }
  }

  return (
    <Card {...stylex.props(styles.card)} padding={4} maxWidth={560} aria-live="polite">
      <strong>{attempt.status === "STARTING" ? "Starting login" : "Waiting for login"}</strong>
      {attempt.verificationUrl ? (
        <Button
          as="a"
          href={attempt.verificationUrl}
          target="_blank"
          rel="noreferrer"
          label="Open login page"
          onClick={openLoginPage}
        >
          Open login page
        </Button>
      ) : null}
      {attempt.userCode ? <code {...stylex.props(styles.userCode)}>{attempt.userCode}</code> : null}
      {attempt.instructions ? <p>{attempt.instructions}</p> : null}
      <p {...stylex.props(styles.continuation)}>Noema will continue automatically.</p>
      <Button type="button" variant="ghost" label="Cancel connection" onClick={onCancel} />
    </Card>
  );
}

const styles = stylex.create({
  card: {
    display: "grid",
    minWidth: 0,
    gap: "calc(var(--spacing-3) + var(--spacing-0-5))"
  },
  userCode: {
    width: "fit-content",
    maxWidth: "100%",
    borderRadius: 6,
    backgroundColor: "var(--surface-sunken)",
    padding: "6px 8px",
    fontFamily: "var(--font-mono)",
    fontSize: 18,
    lineHeight: 1.35,
    whiteSpace: "normal",
    overflowWrap: "anywhere"
  },
  continuation: {
    margin: "var(--spacing-0)",
    fontSize: 14,
    color: "var(--muted-foreground)"
  }
});
