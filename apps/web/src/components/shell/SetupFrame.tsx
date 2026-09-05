import { useEffect, useId, useRef, type ReactNode } from "react";
import { Grid } from "@astryxdesign/core/Grid";
import { VStack } from "@astryxdesign/core/Stack";
import { isTauriRuntime } from "@/graphql/transportMode";
import { IdentityAvatar, LOCAL_AGENT_AVATAR_ID } from "../IdentityAvatar";

export function SetupFrame({ children }: { children: ReactNode }) {
  const desktop = isTauriRuntime();
  return (
    <VStack as="main" height="100dvh" data-setup="frame">
      {desktop ? (
        <header aria-hidden="true" data-tauri-drag-region data-setup="drag" />
      ) : null}
      <VStack data-setup="body" data-desktop={desktop || undefined}>
        {children}
      </VStack>
    </VStack>
  );
}

// The avatar mount and card form one raised surface. Only the avatar moves;
// its centre meets the card edge without changing the title position.
export function SetupCard({
  title,
  intro,
  step,
  children
}: {
  title: string;
  intro: string;
  step?: string;
  children?: ReactNode;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const id = useId();
  useEffect(() => {
    const card = heading.current?.closest("section");
    if (
      card?.contains(document.activeElement) &&
      document.activeElement !== heading.current
    )
      return;
    heading.current?.focus({ preventScroll: true });
  }, [title]);
  return (
    <VStack data-setup="surface">
      <VStack aria-hidden="true" data-setup="avatar">
        <IdentityAvatar
          actorId={LOCAL_AGENT_AVATAR_ID}
          actorType="agent"
          size="setup"
          animated
          focusable={false}
        />
      </VStack>
      <VStack as="section" aria-labelledby={id} gap={4} data-setup="card">
        <VStack as="header" gap={1.5} data-setup="header">
          {step ? <p data-setup="note">{step}</p> : null}
          <h1 id={id} ref={heading} tabIndex={-1} data-setup="title">
            {title}
          </h1>
          <SetupNote>{intro}</SetupNote>
        </VStack>
        <VStack gap={3}>{children}</VStack>
      </VStack>
    </VStack>
  );
}

export function SetupActions({ children }: { children: ReactNode }) {
  return <Grid data-setup="actions">{children}</Grid>;
}

export function SetupNote({ children }: { children: ReactNode }) {
  return <p data-setup="note">{children}</p>;
}
