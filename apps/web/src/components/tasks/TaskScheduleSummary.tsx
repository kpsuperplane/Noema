import type { ReactNode } from "react";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import type { TaskDetail } from "@/components/chatDetail/task/taskTypes";
import { taskScheduleTimestampLabel } from "./tasksModel";

type Schedule = NonNullable<TaskDetail["schedule"]>;

export function TaskScheduleSummary({ schedule, label = "Timing", value, paused = false, children }: {
  schedule?: Schedule | null; label?: string; value?: string; paused?: boolean; children?: ReactNode;
}) {
  return <HStack as="section" align="center" justify="between" gap={2} wrap="wrap" className={stylex.props(styles.summary, paused && styles.paused).className}>
    <VStack gap={1}>
      <span {...stylex.props(styles.label)}>{label}</span>
      <strong {...stylex.props(styles.value)}>{value ?? (schedule ? taskScheduleTimestampLabel(schedule.scheduledFor, schedule.timeZone) : "No scheduled start")}</strong>
    </VStack>
    {children}
  </HStack>;
}

const styles = stylex.create({
  summary: { padding: "var(--spacing-3)", backgroundColor: "var(--noema-pine-50)", color: "var(--noema-pine-700)", borderRadius: "var(--radius-container)" },
  paused: { backgroundColor: "var(--noema-clay-50)", color: "var(--noema-clay-600)" },
  label: { fontSize: "var(--text-supporting-size)", lineHeight: 1.4 },
  value: { fontSize: "var(--text-heading-3-size)", fontFamily: "var(--font-family-heading)", textWrap: "pretty" }
});
