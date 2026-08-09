import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import * as stylex from "@stylexjs/stylex";
import { Plus } from "lucide-react";
import { ShellSectionHeader } from "@/components/shell/ShellSectionHeader";

export function TasksToolbar({ onNewTask }: { onNewTask: () => void }) {
  return (
    <div data-slot="tasks-toolbar" {...stylex.props(styles.toolbar)}>
      <ShellSectionHeader title="Tasks" titleId="tasks-page-title" />
      <Button
        type="button"
        size="sm"
        variant="primary"
        label="New task"
        icon={<Plus aria-hidden="true" size={15} />}
        xstyle={styles.desktopAction}
        onClick={onNewTask}
      />
      <IconButton
        label="New task"
        tooltip="New task"
        size="lg"
        variant="primary"
        icon={<Plus aria-hidden="true" size={20} />}
        xstyle={styles.mobileAction}
        onClick={onNewTask}
      />
    </div>
  );
}

const styles = stylex.create({
  toolbar: {
    position: "relative"
  },
  desktopAction: {
    position: "absolute",
    top: "var(--spacing-3)",
    right: "var(--spacing-4)",
    zIndex: 4,
    "@media (max-width: 760px)": {
      display: "none"
    }
  },
  mobileAction: {
    display: "none",
    "@media (max-width: 760px)": {
      display: "inline-flex",
      position: "fixed",
      right: "max(var(--spacing-4), env(safe-area-inset-right))",
      bottom: "max(var(--spacing-4), env(safe-area-inset-bottom))",
      zIndex: 5,
      width: "var(--spacing-12)",
      height: "var(--spacing-12)",
      "--_button-radius": "var(--radius-full)",
      boxShadow: "var(--shadow-med)"
    }
  }
});
