import * as React from "react";
import * as stylex from "@stylexjs/stylex";

export type ShellPageWidth = "centered" | "fluid";

const ShellPageWidthContext = React.createContext<ShellPageWidth>("centered");

export function ShellPageLayout({
  children,
  width
}: {
  children: React.ReactNode;
  width: ShellPageWidth;
}) {
  return (
    <ShellPageWidthContext.Provider value={width}>
      {children}
    </ShellPageWidthContext.Provider>
  );
}

export function ShellPageTrack({ children }: { children: React.ReactNode }) {
  const width = React.useContext(ShellPageWidthContext);

  return (
    <div
      data-slot="shell-page-track"
      data-page-width={width}
      {...stylex.props(
        styles.track,
        width === "centered" && styles.centered
      )}
    >
      {children}
    </div>
  );
}

export function ShellPageSubtitle({ children }: { children: React.ReactNode }) {
  return <p {...stylex.props(styles.subtitle)}>{children}</p>;
}

const styles = stylex.create({
  track: {
    boxSizing: "content-box",
    minWidth: 0,
    marginInline: "auto",
    paddingInline: "var(--spacing-4)",
    "@media (max-width: 760px)": {
      paddingInline: "var(--spacing-3)"
    }
  },
  centered: {
    maxWidth: "var(--shell-content-max-width)"
  },
  subtitle: {
    maxWidth: 620,
    marginBlock: "0 var(--spacing-3)",
    color: "var(--muted-foreground)",
    fontSize: 14,
    lineHeight: 1.5,
    overflowWrap: "anywhere"
  }
});
