import { Button } from "@astryxdesign/core/Button";
import { TextArea } from "@astryxdesign/core/TextArea";
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
    <form
      {...stylex.props(styles.form)}
      onSubmit={(event) => {
        event.preventDefault();
        if (canSubmit) {
          void onResume(message.trim() || undefined);
        }
      }}
    >
      <div {...stylex.props(styles.copy)}>
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
      </div>
      <TextArea
        isLabelHidden
        label={needsAnswer ? "Answer" : "Guidance"}
        onChange={setMessage}
        placeholder={needsAnswer ? "Type your answer" : "Optional guidance"}
        rows={3}
        value={message}
        width="100%"
      />
      <div {...stylex.props(styles.actions)}>
        <Button
          clickAction={() => onResume(message.trim() || undefined)}
          icon={<ArrowUpRight aria-hidden="true" size={14} />}
          isDisabled={!canSubmit}
          isLoading={busy}
          label="Continue"
          size="sm"
          variant="primary"
        />
      </div>
    </form>
  );
}

export function resumableTaskStatus(status: TaskDetail["status"]): boolean {
  return status === "failed" || status === "waiting_for_human";
}

const styles = stylex.create({
  form: {
    display: "grid",
    gap: 10,
    borderRadius: 8,
    backgroundColor: "var(--noema-surface-sunken)",
    padding: 10
  },
  copy: { display: "grid", gap: 4 },
  title: { color: "var(--noema-text-primary)", fontSize: 12, lineHeight: 1.35 },
  question: {
    margin: 0,
    color: "var(--noema-text-primary)",
    fontSize: 12,
    lineHeight: 1.45,
    textWrap: "pretty"
  },
  description: {
    margin: 0,
    color: "var(--noema-text-secondary)",
    fontSize: 11,
    lineHeight: 1.45,
    textWrap: "pretty"
  },
  actions: { display: "flex", justifyContent: "end" }
});
