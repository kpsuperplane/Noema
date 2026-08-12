import type React from "react";
import { Button } from "@astryxdesign/core/Button";
import { VStack } from "@astryxdesign/core/Stack";
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
    <VStack gap={3} {...stylex.props(styles.root)} aria-live="polite">
      <strong {...stylex.props(styles.status)}>
        {attempt.status === "STARTING" ? "Starting login" : "Waiting for login"}
      </strong>
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
      {attempt.instructions ? <p {...stylex.props(styles.instructions)}>{attempt.instructions}</p> : null}
      <p {...stylex.props(styles.continuation)}>Noema will continue automatically.</p>
      <Button type="button" variant="ghost" label="Cancel connection" onClick={onCancel} />
    </VStack>
  );
}

const styles = stylex.create({
  root: {
    width: "100%",
    maxWidth: 560,
    minWidth: 0,
  },
  status: { textWrap: "balance" },
  instructions: { margin: "var(--spacing-0)", textWrap: "pretty" },
  userCode: {
    width: "fit-content",
    maxWidth: "100%",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--surface-sunken)",
    padding: "var(--spacing-1-5) var(--spacing-2)",
    fontFamily: "var(--font-mono)",
    fontSize: "var(--font-size-lg)",
    lineHeight: 1.35,
    whiteSpace: "normal",
    overflowWrap: "anywhere"
  },
  continuation: {
    margin: "var(--spacing-0)",
    fontSize: "var(--font-size-base)",
    color: "var(--muted-foreground)"
  }
});
