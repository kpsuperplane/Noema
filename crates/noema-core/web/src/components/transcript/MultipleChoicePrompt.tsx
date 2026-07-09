import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput, type CheckboxInputProps } from "@astryxdesign/core/CheckboxInput";
import * as stylex from "@stylexjs/stylex";
import type { MultipleChoiceOption, TurnTranscriptItem } from "@/shared/types";
import { TranscriptChatBubble } from "./TranscriptChatBubble";
import type { ChatBubbleGroup } from "./renderModel";

type MultipleChoicePromptItem = Extract<TurnTranscriptItem, { kind: "multiple_choice_prompt" }>;
type CheckboxXStyle = CheckboxInputProps["xstyle"];

const styles = stylex.create({
  root: {
    display: "grid",
    gap: 8,
    minWidth: 220,
    maxWidth: 520,
    paddingBlockStart: 4,
    paddingBlockEnd: 6,
  },
  prompt: {
    margin: 0,
    color: "inherit",
    font: "inherit",
    lineHeight: "inherit"
  },
  options: {
    display: "grid",
    gap: 4
  },
  option: {
    display: "flex",
    width: "100%",
    minWidth: 0,
    minHeight: 36,
    alignItems: "center",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, currentColor 18%, transparent)",
    borderRadius: 8,
    paddingBlock: 5,
    paddingInline: 8,
    backgroundColor: "color-mix(in srgb, currentColor 6%, transparent)",
    color: "inherit",
    font: "inherit",
    fontSize: 13,
    lineHeight: 1.3,
    cursor: "pointer",
    transition: "background-color 140ms ease, border-color 140ms ease",
    ":hover": {
      backgroundColor: "color-mix(in srgb, currentColor 10%, transparent)"
    },
    ":has(input:disabled)": {
      cursor: "default",
      opacity: 0.84
    },
    ":has(input:disabled):hover": {
      backgroundColor: "color-mix(in srgb, currentColor 6%, transparent)"
    },
    ":has(input:focus-visible)": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, currentColor 44%, transparent)",
      outlineOffset: 2
    }
  },
  selected: {
    borderColor: "color-mix(in srgb, currentColor 42%, transparent)",
    backgroundColor: "color-mix(in srgb, currentColor 14%, transparent)",
    opacity: 1,
    ":hover": {
      backgroundColor: "color-mix(in srgb, currentColor 14%, transparent)"
    }
  },
  buttonOption: {
    appearance: "none",
    gap: 8,
    textAlign: "left",
    ":disabled": {
      cursor: "default",
      opacity: 0.84
    },
    ":disabled:hover": {
      backgroundColor: "color-mix(in srgb, currentColor 6%, transparent)"
    },
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "color-mix(in srgb, currentColor 44%, transparent)",
      outlineOffset: 2
    }
  },
  selectedButtonOption: {
    opacity: 1
  },
  optionIndicator: {
    display: "grid",
    width: 18,
    height: 18,
    flexShrink: 0,
    placeItems: "center",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, currentColor 36%, transparent)",
    borderRadius: 999,
    backgroundColor: "color-mix(in srgb, currentColor 5%, transparent)",
    color: "inherit"
  },
  optionIndicatorSelected: {
    borderColor: "var(--color-accent)",
    backgroundColor: "var(--color-accent)",
    color: "var(--color-on-accent)"
  },
  optionCheck: {
    width: 10,
    height: 10
  },
  optionLabel: {
    minWidth: 0,
    overflowWrap: "anywhere"
  },
  checkbox: {
    width: "100%",
    color: "inherit"
  },
  footer: {
    display: "flex",
    justifyContent: "flex-end"
  }
});

export function MultipleChoicePrompt({
  disabled,
  group,
  item,
  promptItemId,
  submittedSelectedOptionIds,
  showAvatar,
  onSubmit
}: {
  disabled: boolean;
  group?: ChatBubbleGroup;
  item: MultipleChoicePromptItem;
  promptItemId: string;
  submittedSelectedOptionIds: ReadonlySet<string>;
  showAvatar: boolean;
  onSubmit: (promptItemId: string, selectedOptionIds: string[]) => void;
}) {
  const [selectedIds, setSelectedIds] = React.useState<ReadonlySet<string>>(() => new Set());
  const pickMany = item.selection_mode === "PICK_MANY";
  const displayedSelectedIds = disabled ? submittedSelectedOptionIds : selectedIds;

  const toggle = React.useCallback((option: MultipleChoiceOption) => {
    if (disabled) {
      return;
    }
    if (!pickMany) {
      setSelectedIds(new Set([option.id]));
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
  }, [disabled, onSubmit, pickMany, promptItemId]);

  const submitMany = React.useCallback(() => {
    const ids = item.options.filter((option) => selectedIds.has(option.id)).map((option) => option.id);
    if (ids.length > 0) {
      onSubmit(promptItemId, ids);
    }
  }, [item.options, onSubmit, promptItemId, selectedIds]);

  return (
    <TranscriptChatBubble group={group} role="assistant" showAvatar={showAvatar}>
      <div {...stylex.props(styles.root)}>
        <p {...stylex.props(styles.prompt)}>{item.prompt}</p>
        {pickMany ? (
          <div {...stylex.props(styles.options)}>
            {item.options.map((option) => {
              const selected = displayedSelectedIds.has(option.id);
              return (
                <CheckboxInput
                  key={option.id}
                  label={option.label}
                  value={selected}
                  isDisabled={disabled}
                  onChange={() => toggle(option)}
                  size="sm"
                  width="100%"
                  xstyle={checkboxXStyle(styles.option, selected && styles.selected, styles.checkbox)}
                />
              );
            })}
          </div>
        ) : (
          <div {...stylex.props(styles.options)} role="radiogroup" aria-label={item.prompt}>
            {item.options.map((option) => {
              const selected = displayedSelectedIds.has(option.id);
              return (
                <button
                  key={option.id}
                  type="button"
                  disabled={disabled}
                  role="radio"
                  aria-checked={selected}
                  {...stylex.props(
                    styles.option,
                    styles.buttonOption,
                    selected && styles.selected,
                    selected && styles.selectedButtonOption
                  )}
                  onClick={() => toggle(option)}
                >
                  <span {...stylex.props(styles.optionIndicator, selected && styles.optionIndicatorSelected)} aria-hidden="true">
                    {selected ? (
                      <svg viewBox="0 0 10 10" {...stylex.props(styles.optionCheck)}>
                        <path
                          d="M8.5 2.5L4 7.5L1.5 5"
                          stroke="currentColor"
                          strokeWidth="1.5"
                          fill="none"
                          strokeLinecap="round"
                          strokeLinejoin="round"
                        />
                      </svg>
                    ) : null}
                  </span>
                  <span {...stylex.props(styles.optionLabel)}>{option.label}</span>
                </button>
              );
            })}
          </div>
        )}
        {pickMany ? (
          <div {...stylex.props(styles.footer)}>
            <Button
              label="Done"
              size="sm"
              variant="secondary"
              isDisabled={disabled || selectedIds.size === 0}
              onClick={submitMany}
            />
          </div>
        ) : null}
      </div>
    </TranscriptChatBubble>
  );
}

function checkboxXStyle(...xstyle: unknown[]): CheckboxXStyle {
  return xstyle as CheckboxXStyle;
}
