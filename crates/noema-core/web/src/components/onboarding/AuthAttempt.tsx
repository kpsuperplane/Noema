import type React from "react";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { openExternalUrlForAuth } from "@/graphql/externalUrls";
import { isTauriRuntime } from "@/graphql/transportMode";
import type { ProviderAuthAttemptView } from "./types";

export function AuthAttempt({ attempt }: { attempt: ProviderAuthAttemptView }) {
  async function openLoginPage(event: React.MouseEvent<HTMLAnchorElement>) {
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
    <Card className="grid min-w-0 max-w-[560px] gap-2.5" aria-live="polite">
      <CardContent className="grid gap-3.5">
        <strong>{attempt.status === "STARTING" ? "Starting login" : "Waiting for login"}</strong>
        {attempt.verificationUrl ? (
          <Button
            render={
              <a
                href={attempt.verificationUrl}
                target="_blank"
                rel="noreferrer"
                onClick={openLoginPage}
              />
            }
          >
            Open login page
          </Button>
        ) : null}
        {attempt.userCode ? (
          <code className="w-fit max-w-full rounded-md bg-[var(--surface-sunken)] px-2 py-1.5 font-mono text-lg leading-[1.35] whitespace-normal [overflow-wrap:anywhere]">
            {attempt.userCode}
          </code>
        ) : null}
        {attempt.instructions ? <p>{attempt.instructions}</p> : null}
        <p className="m-0 text-sm text-muted-foreground">Noema will continue automatically.</p>
      </CardContent>
    </Card>
  );
}
