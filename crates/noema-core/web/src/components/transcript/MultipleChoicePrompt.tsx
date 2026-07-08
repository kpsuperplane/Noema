import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import type { MultipleChoiceOption, TurnTranscriptItem } from "@/shared/types";

type MultipleChoicePromptItem = Extract<TurnTranscriptItem, { kind: "multiple_choice_prompt" }>;

const styles = stylex.create({
  root: {
    display: "grid",
    gap: 10,
    maxWidth: 520,
    minWidth: 0,
    paddingBlock: 10,
    paddingInline: 12,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 8,
    backgroundColor: "var(--card)",
    boxShadow: "0 8px 28px rgb(15 23 42 / 0.06)"
  },
  prompt: {
    margin: 0,
    color: "var(--foreground)",
    fontSize: 14,
    lineHeight: 1.45
  },
  options: {
    display: "flex",
    flexWrap: "wrap",
    gap: 8
  },
  option: {
    minHeight: 34,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "var(--border)",
    borderRadius: 8,
    paddingBlock: 7,
    paddingInline: 11,
    backgroundColor: "var(--background)",
    color: "var(--foreground)",
    font: "inherit",
    fontSize: 13,
    lineHeight: 1.2,
    cursor: "pointer",
    transition: "background-color 140ms ease, border-color 140ms ease, color 140ms ease",
    ":hover": {
      backgroundColor: "var(--muted)"
    },
    ":disabled": {
      cursor: "default",
      opacity: 0.66
    }
  },
  selected: {
    borderColor: "var(--primary)",
    backgroundColor: "var(--primary)",
    color: "var(--primary-foreground)",
    ":hover": {
      backgroundColor: "var(--primary)"
    }
  },
  footer: {
    display: "flex",
    justifyContent: "flex-end"
  },
  done: {
    minHeight: 34,
    borderWidth: 0,
    borderRadius: 8,
    paddingBlock: 7,
    paddingInline: 12,
    backgroundColor: "var(--primary)",
    color: "var(--primary-foreground)",
    font: "inherit",
    fontSize: 13,
    lineHeight: 1.2,
    cursor: "pointer",
    ":disabled": {
      cursor: "default",
      opacity: 0.5
    }
  }
});

export function MultipleChoicePrompt({
  disabled,
  item,
  promptItemId,
  onSubmit
}: {
  disabled: boolean;
  item: MultipleChoicePromptItem;
  promptItemId: string;
  onSubmit: (promptItemId: string, selectedOptionIds: string[]) => void;
}) {
  const [selectedIds, setSelectedIds] = React.useState<ReadonlySet<string>>(() => new Set());
  const pickMany = item.selection_mode === "PICK_MANY";

  React.useEffect(() => {
    if (disabled) {
      setSelectedIds(new Set());
    }
  }, [disabled]);

  const toggle = React.useCallback((option: MultipleChoiceOption) => {
    if (!pickMany) {
      onSubmit(promptItemId, [option.id]);
      return;
    }
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(option.id)) {
        next.delete(option.id);
      } else {
        next.add(option.id);
      }
      return next;
    });
  }, [onSubmit, pickMany, promptItemId]);

  const submitMany = React.useCallback(() => {
    const ids = item.options.filter((option) => selectedIds.has(option.id)).map((option) => option.id);
    if (ids.length > 0) {
      onSubmit(promptItemId, ids);
    }
  }, [item.options, onSubmit, promptItemId, selectedIds]);

  return (
    <div {...stylex.props(styles.root)}>
      <p {...stylex.props(styles.prompt)}>{item.prompt}</p>
      <div {...stylex.props(styles.options)}>
        {item.options.map((option) => {
          const selected = selectedIds.has(option.id);
          return (
            <button
              key={option.id}
              type="button"
              disabled={disabled}
              aria-pressed={pickMany ? selected : undefined}
              {...stylex.props(styles.option, selected && styles.selected)}
              onClick={() => toggle(option)}
            >
              {option.label}
            </button>
          );
        })}
      </div>
      {pickMany ? (
        <div {...stylex.props(styles.footer)}>
          <button
            type="button"
            disabled={disabled || selectedIds.size === 0}
            {...stylex.props(styles.done)}
            onClick={submitMany}
          >
            Done
          </button>
        </div>
      ) : null}
    </div>
  );
}
