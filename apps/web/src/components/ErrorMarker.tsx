import * as stylex from "@stylexjs/stylex";

const styles = stylex.create({
  root: {
    display: "inline-flex",
    width: "fit-content",
    maxWidth: "100%",
    alignItems: "center",
    borderRadius: 8,
    borderWidth: 1,
    borderStyle: "solid",
    borderColor: "color-mix(in srgb, var(--noema-red-100) 72%, transparent)",
    backgroundColor: "color-mix(in srgb, var(--noema-red-100) 45%, var(--noema-surface-card) 55%)",
    paddingBlock: 4,
    paddingInline: 8,
    color: "var(--noema-red-700)",
    fontSize: 14,
    lineHeight: 1.35
  },
  content: {
    display: "flex",
    minWidth: 0,
    flexWrap: "wrap",
    columnGap: 6,
    overflowWrap: "anywhere"
  }
});

export function ErrorMarker({
  message,
  label,
  recoverable = true
}: {
  message: string;
  label?: string;
  recoverable?: boolean;
}) {
  return (
    <span
      {...stylex.props(styles.root)}
      data-slot="marker"
      data-tone="error"
      role={recoverable ? "status" : "alert"}
    >
      <span {...stylex.props(styles.content)} data-slot="marker-content">
        {label ? <strong>{label}</strong> : null}
        <span>{message}</span>
      </span>
    </span>
  );
}
