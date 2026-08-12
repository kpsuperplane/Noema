import * as stylex from "@stylexjs/stylex";
import {
  List,
  ListItem,
  type ListItemProps,
  type ListProps
} from "@astryxdesign/core/List";
import { HStack } from "@astryxdesign/core/HStack";
import { Section, type SectionProps } from "@astryxdesign/core/Section";
import type { ReactNode } from "react";

type SettingsSectionProps = Omit<SectionProps, "padding" | "variant">;

type SettingsListItemProps = ListItemProps & {
  /** Move selector controls to a full-width line below the setting label on mobile. */
  mobileEndContentFullWidth?: boolean;
};

/**
 * Shared visual boundary for one settings group. The small internal gutter
 * keeps the heading and row labels aligned while the surface makes the group
 * readable as one unit against the settings page.
 */
export function SettingsSection({ xstyle, ...props }: SettingsSectionProps) {
  return (
    <Section
      {...props}
      variant="section"
      padding={3}
      xstyle={[styles.section, xstyle]}
    />
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
    <Section variant="section" padding={0} aria-labelledby={titleId} xstyle={styles.section}>
      <HStack hAlign="between" vAlign="center" gap={2} {...stylex.props(styles.detailHeader)}>
        <h2 id={titleId} {...stylex.props(styles.detailTitle)}>{title}</h2>
        {summary ? (
          <span role={summaryIsStatus ? "status" : undefined} {...stylex.props(styles.detailSummary)}>
            {summary}
          </span>
        ) : null}
      </HStack>
      {children}
    </Section>
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
  detailHeader: { padding: "var(--spacing-2) var(--spacing-3)", borderBlockEndWidth: "var(--border-width)", borderBlockEndStyle: "solid", borderBlockEndColor: "var(--border-subtle)" },
  detailTitle: { minWidth: 0, margin: "var(--spacing-0)", fontFamily: "var(--font-heading)", fontSize: 14, fontWeight: 650 },
  detailSummary: { flexShrink: 0, color: "var(--muted-foreground)", fontSize: 12 },
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
