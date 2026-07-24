import { Button, type ButtonProps } from "@astryxdesign/core/Button";
import * as stylex from "@stylexjs/stylex";
import { Menu } from "lucide-react";
import { useShellSurface } from "./ShellSurfaceContext";

export function ShellSectionHeader({
  description,
  navigationLabel,
  title,
  titleId,
  variant
}: {
  description: string;
  navigationLabel: "Memory" | "Settings";
  title: string;
  titleId?: string;
  variant: "memory" | "settings";
}) {
  const { openSidebar, sidebarOpen, sidebarTriggerRef } = useShellSurface();

  return (
    <header {...stylex.props(styles.header, variant === "memory" && styles.memoryHeader)}>
      <div {...stylex.props(styles.titleRow, variant === "memory" && styles.memoryTitleRow)}>
        <Button
          ref={sidebarTriggerRef}
          type="button"
          variant="ghost"
          size="md"
          label={`Open ${navigationLabel} navigation`}
          icon={<Menu aria-hidden="true" size={18} />}
          aria-controls="noema-shell-sidebar"
          aria-expanded={sidebarOpen}
          xstyle={buttonXStyle(styles.menuButton)}
          onClick={openSidebar}
        />
        <h1
          id={titleId}
          {...stylex.props(
            styles.title,
            variant === "memory" ? styles.memoryTitle : styles.settingsTitle
          )}
        >
          {title}
        </h1>
      </div>
      <p
        {...stylex.props(
          styles.description,
          variant === "memory" ? styles.memoryDescription : styles.settingsDescription
        )}
      >
        {description}
      </p>
    </header>
  );
}

const styles = stylex.create({
  header: {
    display: "grid",
    gap: "var(--spacing-2)"
  },
  memoryHeader: {
    gap: "var(--spacing-1-5)"
  },
  titleRow: {
    display: "flex",
    minWidth: 0,
    alignItems: "center",
    gap: "var(--spacing-2)"
  },
  memoryTitleRow: {
    borderBottomWidth: 1,
    borderBottomStyle: "solid",
    borderBottomColor: "var(--border-default)",
    paddingBottom: "var(--spacing-1-5)"
  },
  menuButton: {
    display: "none",
    flexShrink: 0,
    borderRadius: 999,
    color: "var(--pine-700)",
    "@media (max-width: 760px)": {
      display: "inline-flex"
    }
  },
  title: {
    minWidth: 0,
    margin: 0,
    color: "var(--foreground)",
    overflowWrap: "anywhere"
  },
  settingsTitle: {
    fontFamily: "var(--font-heading)",
    fontSize: 24,
    lineHeight: 1.25,
    letterSpacing: 0
  },
  memoryTitle: {
    fontFamily: "Georgia, 'Times New Roman', serif",
    fontSize: 36,
    fontWeight: 400,
    lineHeight: 1.18,
    "@media (max-width: 760px)": {
      fontSize: 30
    }
  },
  description: {
    margin: 0,
    color: "var(--muted-foreground)"
  },
  settingsDescription: {
    maxWidth: 620,
    fontSize: 14,
    lineHeight: 1.5
  },
  memoryDescription: {
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    fontSize: 12,
    overflowWrap: "anywhere"
  }
});

function buttonXStyle(...xstyle: unknown[]): ButtonProps["xstyle"] {
  return xstyle as unknown as ButtonProps["xstyle"];
}
