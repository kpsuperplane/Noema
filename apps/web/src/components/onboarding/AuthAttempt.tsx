import React from "react";
import { SetupActions, SetupNote } from "../shell/SetupFrame";
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
  const [copyStatus, setCopyStatus] = React.useState("");
  async function copyCode() {
    try {
      await navigator.clipboard.writeText(attempt.userCode ?? "");
      setCopyStatus("Code copied.");
    } catch {
      setCopyStatus("Select the code and copy it manually.");
    }
  }
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
    <VStack gap={3} aria-live="polite">
      {attempt.userCode ? (
        <VStack gap={2} hAlign="center">
          <SetupNote>Enter this code on the sign-in page.</SetupNote>
          <code {...stylex.props(styles.userCode)}>{attempt.userCode}</code>
          <Button
            variant="secondary"
            label="Copy code"
            onClick={() => void copyCode()}
          />
          {copyStatus ? <SetupNote>{copyStatus}</SetupNote> : null}
        </VStack>
      ) : null}
      {attempt.instructions ? (
        <SetupNote>{attempt.instructions}</SetupNote>
      ) : null}
      <SetupActions>
        <Button variant="secondary" label="Cancel sign-in" onClick={onCancel} />
        {attempt.verificationUrl ? (
          <Button
            as="a"
            variant="primary"
            href={attempt.verificationUrl}
            target="_blank"
            rel="noreferrer"
            label="Open sign-in page"
            onClick={openLoginPage}
          />
        ) : null}
      </SetupActions>
      <SetupNote>Noema continues after you finish signing in.</SetupNote>
    </VStack>
  );
}

const styles = stylex.create({
  userCode: {
    maxWidth: "100%",
    borderRadius: "var(--radius-element)",
    backgroundColor: "var(--surface-sunken)",
    padding: "var(--spacing-2) var(--spacing-3)",
    fontFamily: "var(--font-mono)",
    fontSize: "var(--font-size-lg)",
    overflowWrap: "anywhere",
    textAlign: "center"
  }
});
