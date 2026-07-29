import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { TextArea } from "@astryxdesign/core/TextArea";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { ArrowUpRight } from "lucide-react";
import * as React from "react";
import type { TaskDetail } from "./taskTypes";

export function TaskResumeControls({
  detail,
  busy,
  onResume
}: {
  detail: TaskDetail;
  busy: boolean;
  onResume: (message?: string) => void | Promise<void>;
}) {
  const [message, setMessage] = React.useState("");
  const needsAnswer = detail.status === "waiting_for_human";
  const canSubmit = !busy && (!needsAnswer || Boolean(message.trim()));

  if (!(detail.canResume ?? resumableTaskStatus(detail.status))) {
    return null;
  }

  return (
    <VStack
      as="form"
      gap={3}
      className={stylex.props(styles.form).className}
      onSubmit={(event) => {
        event.preventDefault();
        if (canSubmit) {
          void onResume(message.trim() || undefined);
        }
      }}
    >
      <VStack gap={1}>
        <strong {...stylex.props(styles.title)}>
          {needsAnswer ? "Answer the executor" : "Continue this task"}
        </strong>
        {detail.blockingQuestion ? (
          <p {...stylex.props(styles.question)}>{detail.blockingQuestion}</p>
        ) : (
          <p {...stylex.props(styles.description)}>
            Continue from the saved transcript and evidence. Add guidance if it would help.
          </p>
        )}
      </VStack>
      <TextArea
        isLabelHidden
        label={needsAnswer ? "Answer" : "Guidance"}
        onChange={setMessage}
        placeholder={needsAnswer ? "Type your answer" : "Optional guidance"}
        rows={3}
        value={message}
        width="100%"
      />
      <HStack justify="start">
        <Button
          clickAction={() => onResume(message.trim() || undefined)}
          icon={<ArrowUpRight aria-hidden="true" size={14} />}
          isDisabled={!canSubmit}
          isLoading={busy}
          label="Continue"
          size="sm"
          variant="primary"
        />
      </HStack>
    </VStack>
  );
}

export function resumableTaskStatus(status: TaskDetail["status"]): boolean {
  return status === "failed" || status === "waiting_for_human";
}

const styles = stylex.create({
  form: {
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-sunken)",
    padding: "calc(var(--spacing-2) + var(--spacing-0-5))"
  },
  title: { color: "var(--noema-text-primary)", fontSize: 12, lineHeight: 1.35 },
  question: {
    margin: "var(--spacing-0)",
    color: "var(--noema-text-primary)",
    fontSize: 12,
    lineHeight: 1.45,
    textWrap: "pretty"
  },
  description: {
    margin: "var(--spacing-0)",
    color: "var(--noema-text-secondary)",
    fontSize: 11,
    lineHeight: 1.45,
    textWrap: "pretty"
  },
});
