import { Popover } from "@astryxdesign/core/Popover";
import * as stylex from "@stylexjs/stylex";
import { ChevronDown } from "lucide-react";
import { useState } from "react";
import { MemoryCitationDetails } from "@/pages/MemoryCitationDetails";
import type { MemoryArticleReference } from "@/pages/memoryPageModel";

export function MemoryCitation({
  number,
  reference
}: {
  number: number;
  reference: MemoryArticleReference;
}) {
  const [open, setOpen] = useState(false);

  return (
    <li {...stylex.props(styles.item)}>
      <span {...stylex.props(styles.citationLine)}>
        <Popover
          alignment="start"
          closeButtonLabel={`Close citation ${number} details`}
          content={<MemoryCitationDetails number={number} reference={reference} />}
          hasAutoFocus={false}
          isOpen={open}
          label={`Citation ${number} details`}
          onOpenChange={setOpen}
          placement="below"
          width="min(430px, calc(100vw - 32px))"
        >
          {(triggerProps) => (
            <button
              {...triggerProps}
              type="button"
              {...stylex.props(styles.citationLink)}
            >
              <span>{reference.label}</span>
              <ChevronDown
                aria-hidden="true"
                size={13}
                strokeWidth={1.8}
                {...stylex.props(styles.expandIcon, open && styles.expandIconOpen)}
              />
            </button>
          )}
        </Popover>
        <span {...stylex.props(styles.citationContext)}>
          {" "}
          Noema private memory record
          {reference.memoryUpdatedAtLabel
            ? `, memory updated ${reference.memoryUpdatedAtLabel}`
            : ""}
          .
        </span>
      </span>
    </li>
  );
}

const styles = stylex.create({
  item: {
    minWidth: 0,
    paddingLeft: 2,
    overflowWrap: "anywhere"
  },
  citationLine: {
    color: "#202122"
  },
  citationLink: {
    display: "inline-flex",
    alignItems: "center",
    gap: 2,
    maxWidth: "100%",
    borderWidth: 0,
    borderRadius: 2,
    backgroundColor: "transparent",
    padding: 0,
    color: "#36c",
    font: "inherit",
    lineHeight: "inherit",
    textAlign: "left",
    textDecoration: "none",
    cursor: "pointer",
    transitionDuration: "120ms",
    transitionProperty: "color",
    ":hover": {
      color: "#233f8f",
      textDecoration: "underline"
    },
    ":focus-visible": {
      outlineWidth: 2,
      outlineStyle: "solid",
      outlineColor: "#36c",
      outlineOffset: 2
    }
  },
  citationContext: {
    color: "#54595d"
  },
  expandIcon: {
    flexShrink: 0,
    transitionDuration: "140ms",
    transitionProperty: "transform"
  },
  expandIconOpen: {
    transform: "rotate(180deg)"
  }
});
