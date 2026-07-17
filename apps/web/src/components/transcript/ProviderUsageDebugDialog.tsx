import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import * as stylex from "@stylexjs/stylex";
import { providerUsageDebugRows, type ProviderUsageDebug } from "./debugUsage";

const styles = stylex.create({
  body: {
    display: "grid",
    gap: 14,
    padding: 18
  },
  details: {
    display: "grid",
    gridTemplateColumns: "minmax(120px, max-content) minmax(0, 1fr)",
    columnGap: 16,
    rowGap: 9,
    margin: 0
  },
  label: {
    fontFamily: "var(--noema-font-mono)",
    fontSize: 10,
    letterSpacing: "0.08em",
    textTransform: "uppercase",
    color: "var(--noema-text-faint)"
  },
  value: {
    margin: 0,
    fontSize: 13,
    color: "var(--noema-text-primary)",
    overflowWrap: "anywhere"
  }
});

export function ProviderUsageDebugDialog({
  debug,
  open,
  onOpenChange
}: {
  debug: ProviderUsageDebug | null;
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  return (
    <Dialog
      isOpen={open}
      onOpenChange={onOpenChange}
      purpose="info"
      width={460}
      aria-label="Provider debug details"
    >
      <DialogHeader title="Debug" subtitle="Provider usage" onOpenChange={onOpenChange} />
      <div {...stylex.props(styles.body)}>
        {debug ? (
          <dl {...stylex.props(styles.details)}>
            {providerUsageDebugRows(debug).map((row) => (
              <div key={row.label}>
                <dt {...stylex.props(styles.label)}>{row.label}</dt>
                <dd {...stylex.props(styles.value)}>{row.value}</dd>
              </div>
            ))}
          </dl>
        ) : null}
      </div>
    </Dialog>
  );
}
