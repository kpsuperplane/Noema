import * as React from "react";
import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput, type CheckboxInputProps } from "@astryxdesign/core/CheckboxInput";
import { RadioList, RadioListItem, type RadioListItemProps, type RadioListProps } from "@astryxdesign/core/RadioList";
import * as stylex from "@stylexjs/stylex";
import type { MultipleChoiceOption, TurnTranscriptItem } from "@/shared/types";
import { TranscriptChatBubble } from "./TranscriptChatBubble";

type MultipleChoicePromptItem = Extract<TurnTranscriptItem, { kind: "multiple_choice_prompt" }>;
type CheckboxXStyle = CheckboxInputProps["xstyle"];
type RadioListXStyle = RadioListProps["xstyle"];
type RadioListItemXStyle = RadioListItemProps["xstyle"];

const emptySelectedIds: ReadonlySet<string> = new Set();

const styles = stylex.create({
  root: {
    display: "grid",
    gap: 8,
    minWidth: 220,
    maxWidth: 520
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
      opacity: 0.64
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
    ":hover": {
      backgroundColor: "color-mix(in srgb, currentColor 14%, transparent)"
    }
  },
  checkbox: {
    width: "100%",
    color: "inherit"
  },
  radioList: {
    color: "inherit"
  },
  radioItem: {
    width: "100%",
    color: "inherit",
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, currentColor 18%, transparent)",
    borderRadius: 8,
    backgroundColor: "color-mix(in srgb, currentColor 6%, transparent)",
    ":hover": {
      backgroundColor: "color-mix(in srgb, currentColor 10%, transparent)"
    }
  },
  radioItemSelected: {
    borderColor: "color-mix(in srgb, currentColor 42%, transparent)",
    backgroundColor: "color-mix(in srgb, currentColor 14%, transparent)",
    ":hover": {
      backgroundColor: "color-mix(in srgb, currentColor 14%, transparent)"
    }
  },
  footer: {
    display: "flex",
    justifyContent: "flex-end"
  }
});

export function MultipleChoicePrompt({
  disabled,
  item,
  promptItemId,
  showAvatar,
  onSubmit
}: {
  disabled: boolean;
  item: MultipleChoicePromptItem;
  promptItemId: string;
  showAvatar: boolean;
  onSubmit: (promptItemId: string, selectedOptionIds: string[]) => void;
}) {
  const [selectedIds, setSelectedIds] = React.useState<ReadonlySet<string>>(() => new Set());
  const pickMany = item.selection_mode === "PICK_MANY";
  const displayedSelectedIds = disabled ? emptySelectedIds : selectedIds;

  const toggle = React.useCallback((option: MultipleChoiceOption) => {
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
  }, [onSubmit, pickMany, promptItemId]);

  const submitMany = React.useCallback(() => {
    const ids = item.options.filter((option) => selectedIds.has(option.id)).map((option) => option.id);
    if (ids.length > 0) {
      onSubmit(promptItemId, ids);
    }
  }, [item.options, onSubmit, promptItemId, selectedIds]);

  const selectedRadioId = pickMany ? "" : [...displayedSelectedIds][0] ?? "";

  return (
    <TranscriptChatBubble role="assistant" showAvatar={showAvatar}>
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
          <RadioList
            label={item.prompt}
            isLabelHidden
            value={selectedRadioId}
            onChange={(value) => {
              const option = item.options.find((candidate) => candidate.id === value);
              if (option) {
                toggle(option);
              }
            }}
            isDisabled={disabled}
            size="sm"
            width="100%"
            xstyle={radioListXStyle(styles.radioList)}
          >
            {item.options.map((option) => {
              const selected = displayedSelectedIds.has(option.id);
              return (
                <RadioListItem
                  key={option.id}
                  label={option.label}
                  value={option.id}
                  xstyle={radioListItemXStyle(styles.radioItem, selected && styles.radioItemSelected)}
                />
              );
            })}
          </RadioList>
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

function radioListXStyle(...xstyle: unknown[]): RadioListXStyle {
  return xstyle as RadioListXStyle;
}

function radioListItemXStyle(...xstyle: unknown[]): RadioListItemXStyle {
  return xstyle as RadioListItemXStyle;
}
