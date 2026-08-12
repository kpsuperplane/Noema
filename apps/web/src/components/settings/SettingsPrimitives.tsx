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
import { createContext, type ReactNode, useContext } from "react";

type SettingsBodyTreatment = "outside" | "edge" | "inset";

const SettingsBodyTreatmentContext = createContext<SettingsBodyTreatment>("outside");

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
      <SettingsBodyTreatmentContext value={hasHeader ? "edge" : "inset"}>
        {children}
      </SettingsBodyTreatmentContext>
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
      <SettingsBodyTreatmentContext value="inset">{children}</SettingsBodyTreatmentContext>
    </VStack>
  );
}

export function SettingsSectionBody({
  divided = false,
  children
}: {
  divided?: boolean;
  children: ReactNode;
}) {
  return <VStack {...stylex.props(divided && styles.divided)}>{children}</VStack>;
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
      <VStack gap={2} {...stylex.props(styles.technicalBody)}>
        <SettingsBodyTreatmentContext value="inset">{children}</SettingsBodyTreatmentContext>
      </VStack>
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

export function SettingsLocalFeedback({ children }: { children: ReactNode }) {
  return (
    <VStack gap={1} {...stylex.props(styles.feedback)}>
      <SettingsBodyTreatmentContext value="inset">{children}</SettingsBodyTreatmentContext>
    </VStack>
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
  className,
  xstyle,
  label,
  description,
  endContent,
  style,
  mobileEndContentFullWidth = false,
  ...props
}: SettingsListItemProps) {
  const bodyTreatment = useContext(SettingsBodyTreatmentContext);
  const fullWidthContent = mobileEndContentFullWidth ? (
    <span {...stylex.props(styles.fullWidthContent)}>
      <span {...stylex.props(styles.fullWidthCopy)}>
        <span>{label}</span>
        {description != null ? <span {...stylex.props(styles.fullWidthDescription)}>{description}</span> : null}
      </span>
      {endContent != null ? <span {...stylex.props(styles.fullWidthActions)}>{endContent}</span> : null}
    </span>
  ) : null;
  return (
    <ListItem
      {...props}
      label={fullWidthContent ?? label}
      description={mobileEndContentFullWidth ? undefined : description}
      endContent={mobileEndContentFullWidth ? undefined : endContent}
      className={[className, stylex.props(styles.lastItem).className].filter(Boolean).join(" ")}
      xstyle={[
        bodyTreatment === "edge" ? styles.edgeItem : styles.item,
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
  feedback: {
    padding: "var(--spacing-2) var(--spacing-3)",
    borderBlockStartWidth: "var(--border-width)",
    borderBlockStartStyle: "solid",
    borderBlockStartColor: "var(--border-subtle)"
  },
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
  edgeItem: {
    paddingInline: "var(--spacing-3)",
    flexWrap: "nowrap"
  },
  lastItem: {
    ":last-child": { borderBlockEndWidth: 0 }
  },
  fullWidthContent: {
    display: "grid",
    gridTemplateColumns: "minmax(0, 1fr) auto",
    alignItems: "center",
    columnGap: "var(--spacing-2)",
    minWidth: 0,
    "@media (max-width: 620px)": {
      gridTemplateColumns: "minmax(0, 1fr)",
      rowGap: "var(--spacing-1)"
    }
  },
  fullWidthCopy: { display: "flex", flexDirection: "column", minWidth: 0 },
  fullWidthDescription: {
    color: "var(--muted-foreground)",
    fontSize: 12,
    lineHeight: 1.4,
    overflowWrap: "anywhere"
  },
  fullWidthActions: {
    minWidth: 0,
    justifySelf: "end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifySelf: "stretch"
    }
  }
});
