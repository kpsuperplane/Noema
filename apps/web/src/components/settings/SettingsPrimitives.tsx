import * as stylex from "@stylexjs/stylex";
import {
  List,
  ListItem,
  type ListItemProps,
  type ListProps
} from "@astryxdesign/core/List";
import { Section, type SectionProps } from "@astryxdesign/core/Section";

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
      xstyle={asCoreXStyle<SectionProps["xstyle"]>(styles.section, xstyle)}
    />
  );
}

export function SettingsList({ xstyle, ...props }: ListProps) {
  return <List {...props} xstyle={asCoreXStyle<ListProps["xstyle"]>(styles.list, xstyle)} />;
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
      xstyle={asCoreXStyle<ListItemProps["xstyle"]>(
        mobileEndContentFullWidth ? styles.itemMobile : styles.item,
        xstyle
      )}
      style={style}
    />
  );
}

function asCoreXStyle<T>(...values: unknown[]) {
  return values.filter(Boolean) as T;
}

const styles = stylex.create({
  section: {
    borderWidth: "var(--border-width)",
    borderStyle: "solid",
    borderColor: "var(--border-subtle)",
    borderRadius: "var(--radius-container)",
    overflow: "hidden"
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
