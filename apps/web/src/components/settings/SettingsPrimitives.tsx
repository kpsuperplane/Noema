import * as stylex from "@stylexjs/stylex";
import {
  List,
  ListItem,
  type ListItemProps,
  type ListProps
} from "@astryxdesign/core/List";
import { HStack } from "@astryxdesign/core/HStack";
import { Section, type SectionProps } from "@astryxdesign/core/Section";
import { VStack } from "@astryxdesign/core/VStack";
import type { ReactNode } from "react";

type SettingsSectionProps = Omit<SectionProps, "padding" | "variant" | "title"> & {
  title?: string;
  titleId?: string;
  summary?: ReactNode;
  action?: ReactNode;
};

type SettingsListItemProps = ListItemProps & {
  /** Move selector controls to a full-width line below the setting label on mobile. */
  mobileEndContentFullWidth?: boolean;
};

/**
 * Shared visual boundary for one settings group. The small internal gutter
 * keeps the heading and row labels aligned while the surface makes the group
 * readable as one unit against the settings page.
 */
export function SettingsSection({
  title,
  titleId,
  summary,
  action,
  children,
  xstyle,
  ...props
}: SettingsSectionProps) {
  const hasHeader = Boolean(title && titleId);
  return (
    <Section
      {...props}
      variant="section"
      padding={hasHeader ? 0 : 3}
      aria-labelledby={hasHeader ? titleId : props["aria-labelledby"]}
      xstyle={[styles.section, xstyle]}
    >
      {hasHeader ? (
        <HStack hAlign="between" vAlign="center" gap={2} wrap="wrap" {...stylex.props(styles.header)}>
          <h2 id={titleId} {...stylex.props(styles.title)}>{title}</h2>
          {action ?? (summary ? <span {...stylex.props(styles.summary)}>{summary}</span> : null)}
        </HStack>
      ) : null}
      {children}
    </Section>
  );
}

export function SettingsSectionInset({
  divided = false,
  children
}: {
  divided?: boolean;
  children: ReactNode;
}) {
  return (
    <VStack gap={2} {...stylex.props(styles.inset, divided && styles.divided)}>
      {children}
    </VStack>
  );
}

export function SettingsTechnicalDetails({
  summary = "Technical details",
  children
}: {
  summary?: string;
  children: ReactNode;
}) {
  return (
    <details {...stylex.props(styles.technicalDetails)}>
      <summary {...stylex.props(styles.technicalSummary)}>{summary}</summary>
      <VStack gap={2} {...stylex.props(styles.technicalBody)}>{children}</VStack>
    </details>
  );
}

export function SettingsRowActions({ children }: { children: ReactNode }) {
  return (
    <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowActions)}>
      {children}
    </HStack>
  );
}

export function SettingsDetailSection({
  title,
  titleId,
  summary,
  summaryIsStatus = false,
  children
}: {
  title: string;
  titleId: string;
  summary?: string;
  summaryIsStatus?: boolean;
  children: ReactNode;
}) {
  return (
    <SettingsSection
      title={title}
      titleId={titleId}
      summary={summary ? (
        <span role={summaryIsStatus ? "status" : undefined}>{summary}</span>
      ) : undefined}
    >
      {children}
    </SettingsSection>
  );
}

export function SettingsList({ xstyle, ...props }: ListProps) {
  return <List {...props} xstyle={[styles.list, xstyle]} />;
}

export function SettingsListItem({
  xstyle,
  endContent,
  style,
  mobileEndContentFullWidth = false,
  ...props
}: SettingsListItemProps) {
  return (
    <ListItem
      {...props}
      endContent={
        endContent == null || !mobileEndContentFullWidth ? endContent : (
          <span {...stylex.props(styles.endContent)}>{endContent}</span>
        )
      }
      xstyle={[
        mobileEndContentFullWidth ? styles.itemMobile : styles.item,
        xstyle
      ]}
      style={style}
    />
  );
}

const styles = stylex.create({
  section: {
    borderWidth: "var(--border-width)",
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: "var(--radius-container)",
    overflow: "hidden"
  },
  header: {
    padding: "var(--spacing-2) var(--spacing-3)",
    borderBlockEndWidth: "var(--border-width)",
    borderBlockEndStyle: "solid",
    borderBlockEndColor: "var(--border-subtle)"
  },
  title: {
    minWidth: 0,
    margin: "var(--spacing-0)",
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 14,
    fontWeight: 650,
    lineHeight: 1.3
  },
  summary: { flexShrink: 0, color: "var(--muted-foreground)", fontSize: 12 },
  inset: { padding: "var(--spacing-3)" },
  divided: {
    borderBlockStartWidth: "var(--border-width)",
    borderBlockStartStyle: "solid",
    borderBlockStartColor: "var(--border-subtle)"
  },
  technicalDetails: {
    padding: "var(--spacing-2) var(--spacing-3)",
    borderBlockStartWidth: "var(--border-width)",
    borderBlockStartStyle: "solid",
    borderBlockStartColor: "var(--border-subtle)"
  },
  technicalSummary: { color: "var(--muted-foreground)", fontSize: 12, fontWeight: 600 },
  technicalBody: { paddingBlockStart: "var(--spacing-2)" },
  rowActions: {
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start"
    }
  },
  list: {
    marginBlock: "var(--spacing-0)"
  },
  item: {
    paddingInline: "var(--spacing-0)",
    flexWrap: "nowrap"
  },
  itemMobile: {
    paddingInline: "var(--spacing-0)",
    "@media (max-width: 620px)": {
      flexWrap: "wrap",
      alignItems: "flex-start",
      rowGap: "var(--spacing-1)"
    }
  },
  endContent: {
    "@media (max-width: 620px)": {
      width: "calc(100vw - var(--spacing-6) - var(--spacing-6) - var(--spacing-0-5) + var(--spacing-2))",
      maxWidth: "100%",
      minWidth: 0
    }
  }
});
