import { Dialog as AstryxDialog, DialogHeader } from "@astryxdesign/core/Dialog";
import type { DialogProps } from "@astryxdesign/core/Dialog";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import * as stylex from "@stylexjs/stylex";
import { AnimatePresence, useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { springs } from "@/motion/springs";
import {
  MobileDrawer,
  drawerDimension,
  useLatchedDrawerPresentation
} from "./MobileDrawer";

const mobileViewport = "(max-width: 760px)";

export function Dialog(props: DialogProps) {
  const isMobile = useMediaQuery(mobileViewport);
  const renderAsMobile = useLatchedDrawerPresentation(props.isOpen, isMobile) && props.variant !== "fullscreen";

  if (!renderAsMobile) {
    return <NativeDialog {...props} />;
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

export function NativeDialog(props: DialogProps) {
  return (
    <AnimatePresence propagate>
      {props.isOpen && <AnimatedDialog {...props} />}
    </AnimatePresence>
  );
}

function AnimatedDialog(props: DialogProps) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();
  return (
    // This wrapper owns presence without replacing the native dialog's ref.
    <m.div
      style={{ display: "contents" }}
      initial={reduceMotion ? false : { "--dialog-progress": 0 }}
      animate={{ "--dialog-progress": 1 }}
      exit={{ "--dialog-progress": 0 }}
      transition={reduceMotion ? { duration: 0 } : springs.surface}
    >
      <AstryxDialog {...props} isOpen={isPresent} inert={!isPresent} aria-hidden={!isPresent || undefined} />
    </m.div>
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
