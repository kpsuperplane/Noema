import { Dialog as AstryxDialog, DialogHeader } from "@astryxdesign/core/Dialog";
import type { DialogProps } from "@astryxdesign/core/Dialog";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import * as stylex from "@stylexjs/stylex";
import {
  MobileDrawer,
  drawerDimension,
  useLatchedDrawerPresentation
} from "./MobileDrawer";

const mobileViewport = "(max-width: 760px)";

export function Dialog(props: DialogProps) {
  const isMobile = useMediaQuery(mobileViewport);
  const renderAsMobile = useLatchedDrawerPresentation(props.isOpen, isMobile);

  if (!renderAsMobile) {
    return <AstryxDialog {...props} />;
  }

  const {
    isOpen,
    onOpenChange,
    purpose = "info",
    maxHeight,
    padding,
    children,
  } = props;
  const label = props["aria-label"] ?? "Dialog";
  const automaticDismissal = purpose === "info";
  const drawerMaxHeight = drawerDimension(maxHeight);
  const dialogMaxHeight = `calc(${drawerMaxHeight} - var(--spacing-5) - env(safe-area-inset-bottom, 0px))`;

  return (
    <MobileDrawer
      isOpen={isOpen}
      onOpenChange={onOpenChange}
      label={label}
      dismissible={automaticDismissal}
      closeOnEscape={purpose === "form"}
      maxHeight={drawerMaxHeight}
      role={purpose === "required" ? "alertdialog" : "dialog"}
    >
      <AstryxDialog
        isOpen
        isInline
        onOpenChange={onOpenChange}
        purpose={purpose}
        padding={padding}
        width="100%"
        maxHeight={dialogMaxHeight}
        aria-label={label}
        className={stylex.props(styles.inlineDialog).className}
      >
        {children}
      </AstryxDialog>
    </MobileDrawer>
  );
}

export { DialogHeader };
export type { DialogProps };

const styles = stylex.create({
  inlineDialog: {
    width: "100%",
    maxWidth: "none",
    minHeight: 0,
    overflowY: "auto",
    borderRadius: 0,
    boxShadow: "none"
  }
});
