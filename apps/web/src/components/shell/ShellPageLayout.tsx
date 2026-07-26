import * as React from "react";
import * as stylex from "@stylexjs/stylex";

export type ShellPageWidth = "fluid" | "standard" | "reading";

const ShellPageWidthContext = React.createContext<ShellPageWidth>("fluid");

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
        width === "standard" && styles.standard,
        width === "reading" && styles.reading
      )}
    >
      {children}
    </div>
  );
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
  standard: {
    maxWidth: 768
  },
  reading: {
    maxWidth: 1020
  }
});
