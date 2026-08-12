import type { ReactNode } from "react";
import { Button } from "@astryxdesign/core/Button";
import { IconButton } from "@astryxdesign/core/IconButton";
import { VStack } from "@astryxdesign/core/VStack";
import { Plus } from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import {
  MasterDetailLayout,
  useDetailPanePresentation
} from "@/components/shell/MasterDetailLayout";
import { ShellSectionHeader } from "@/components/shell/ShellSectionHeader";

export type SettingsPrimaryAction = {
  label: string;
  onClick: () => void;
};

export function SettingsManagementLayout({
  title,
  primaryAction,
  list,
  detail,
  detailOpen,
  detailLabel,
  onDetailOpenChange
}: {
  title: string;
  primaryAction?: SettingsPrimaryAction;
  list: ReactNode;
  detail?: ReactNode;
  detailOpen: boolean;
  detailLabel: string;
  onDetailOpenChange: (open: boolean) => void;
}) {
  return (
    <MasterDetailLayout
      detailOpen={detailOpen}
      detailLabel={detailLabel}
      onDetailOpenChange={onDetailOpenChange}
      list={
        <VStack {...stylex.props(styles.scroller)}>
          <ManagementToolbar title={title} action={primaryAction} />
          <VStack {...stylex.props(styles.listContent)}>{list}</VStack>
        </VStack>
      }
      detail={detail ? (
        <VStack {...stylex.props(styles.scroller)}>
          <ManagementDetailBody>{detail}</ManagementDetailBody>
        </VStack>
      ) : null}
    />
  );
}

function ManagementDetailBody({ children }: { children: ReactNode }) {
  const isDrawer = useDetailPanePresentation() === "drawer";
  return (
    <VStack gap={3} {...stylex.props(styles.detailContent, isDrawer && styles.drawerDetailContent)}>
      {children}
    </VStack>
  );
}

function ManagementToolbar({
  title,
  action
}: {
  title: string;
  action?: SettingsPrimaryAction;
}) {
  return (
    <header {...stylex.props(styles.toolbar)}>
      <ShellSectionHeader title={title} titleId="settings-surface-title" />
      {action ? <>
        <Button
          type="button"
          size="sm"
          variant="primary"
          label={action.label}
          icon={<Plus aria-hidden="true" size={15} />}
          xstyle={styles.desktopAction}
          onClick={action.onClick}
        />
        <IconButton
          label={action.label}
          tooltip={action.label}
          size="lg"
          variant="primary"
          icon={<Plus aria-hidden="true" size={20} />}
          xstyle={styles.mobileAction}
          onClick={action.onClick}
        />
      </> : null}
    </header>
  );
}

const styles = stylex.create({
  scroller: {
    width: "100%",
    height: "100%",
    minHeight: 0,
    overflowY: "auto",
    overflowX: "hidden",
    overscrollBehavior: "contain",
    scrollbarWidth: "thin"
  },
  listContent: {
    minWidth: 0,
    padding: "var(--spacing-4)",
    "@media (max-width: 760px)": { padding: "var(--spacing-3)" }
  },
  toolbar: { position: "sticky", top: 0, zIndex: 3, flexShrink: 0 },
  desktopAction: {
    position: "absolute",
    top: "var(--spacing-3)",
    right: "var(--spacing-4)",
    zIndex: 4,
    "@media (max-width: 760px)": { display: "none" }
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
  },
  detailContent: { minWidth: 0, paddingBlock: "var(--spacing-4)", paddingInline: "var(--spacing-4)" },
  drawerDetailContent: { paddingBlock: "var(--spacing-0)", paddingInline: "var(--spacing-2)" }
});
