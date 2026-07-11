import { Badge } from "@astryxdesign/core/Badge";
import * as stylex from "@stylexjs/stylex";
import { Check, Clock3, MailWarning, Minus } from "lucide-react";
import type { ReactNode } from "react";
import type { TaskDelivery as TaskDeliveryState } from "./taskTypes";

export function TaskDelivery({ delivery }: { delivery?: TaskDeliveryState | null }) {
  if (!delivery) {
    return null;
  }
  const meta = deliveryMeta(delivery.status);
  return (
    <section aria-labelledby="task-delivery-title" {...stylex.props(styles.section)}>
      <div {...stylex.props(styles.heading)}>
        <h3 id="task-delivery-title" {...stylex.props(styles.title)}>Completion delivery</h3>
        <Badge variant={meta.variant} icon={meta.icon} label={meta.label} {...stylex.props(styles.badge)} />
      </div>
      <dl {...stylex.props(styles.metadata)}>
        {delivery.destination ? <MetadataRow label="Destination" value={delivery.destination} /> : null}
        {delivery.deliveredAt ? <MetadataRow label="Delivered" value={formatDate(delivery.deliveredAt)} /> : null}
      </dl>
      {delivery.error ? <p role="alert" {...stylex.props(styles.error)}>{delivery.error}</p> : null}
    </section>
  );
}

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div {...stylex.props(styles.metadataRow)}>
      <dt {...stylex.props(styles.metadataLabel)}>{label}</dt>
      <dd {...stylex.props(styles.metadataValue)}>{value}</dd>
    </div>
  );
}

function deliveryMeta(status: TaskDeliveryState["status"]): {
  label: string;
  variant: "neutral" | "success" | "warning" | "error";
  icon: ReactNode;
} {
  const iconProps = { "aria-hidden": true, size: 12, strokeWidth: 2 } as const;
  switch (status) {
    case "delivered":
      return { label: "Delivered", variant: "success", icon: <Check {...iconProps} /> };
    case "failed":
      return { label: "Delivery failed", variant: "error", icon: <MailWarning {...iconProps} /> };
    case "pending":
      return { label: "Pending", variant: "warning", icon: <Clock3 {...iconProps} /> };
    case "not_required":
      return { label: "Not required", variant: "neutral", icon: <Minus {...iconProps} /> };
  }
}

function formatDate(value: string): string {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp)
    ? value
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(timestamp);
}

const styles = stylex.create({
  section: { display: "grid", gap: 9, paddingBlock: 2 },
  heading: { display: "flex", flexWrap: "wrap", alignItems: "center", justifyContent: "space-between", gap: 8 },
  title: { margin: 0, color: "var(--noema-text-primary)", fontSize: 13, fontWeight: 700, lineHeight: 1.35 },
  badge: { flexShrink: 0, fontSize: 10 },
  metadata: { display: "grid", gap: 6, margin: 0 },
  metadataRow: { display: "grid", gridTemplateColumns: "minmax(82px, 0.35fr) minmax(0, 1fr)", gap: 8 },
  metadataLabel: { color: "var(--noema-text-muted)", fontSize: 11 },
  metadataValue: { minWidth: 0, margin: 0, color: "var(--noema-text-secondary)", fontSize: 11, overflowWrap: "anywhere" },
  error: { margin: 0, color: "var(--noema-red-700)", fontSize: 11, lineHeight: 1.4, overflowWrap: "anywhere" }
});
