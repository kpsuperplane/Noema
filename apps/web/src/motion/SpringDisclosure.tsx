import type { ReactNode } from "react";
import { AnimatePresence, useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import * as stylex from "@stylexjs/stylex";
import { springs } from "./springs";

const styles = stylex.create({
  content: {
    minHeight: 0,
    overflow: "hidden"
  }
});

export function SpringDisclosure({
  children,
  className,
  id,
  open,
  slot
}: {
  children: ReactNode;
  className?: string;
  id?: string;
  open: boolean;
  slot?: string;
}) {
  return (
    <AnimatePresence initial={false}>
      {open ? (
        <SpringDisclosureContent className={className} id={id} slot={slot}>
          {children}
        </SpringDisclosureContent>
      ) : null}
    </AnimatePresence>
  );
}

function SpringDisclosureContent({
  children,
  className,
  id,
  slot
}: Omit<Parameters<typeof SpringDisclosure>[0], "open">) {
  const isPresent = useIsPresent();
  const reduceMotion = useReducedMotion();

  return (
    <m.div
      id={id}
      className={className}
      data-slot={slot}
      aria-hidden={isPresent ? undefined : "true"}
      inert={!isPresent}
      initial={reduceMotion ? false : { gridTemplateRows: "0fr", opacity: 0, y: -4 }}
      animate={{ gridTemplateRows: "1fr", opacity: 1, y: 0 }}
      exit={{ gridTemplateRows: "0fr", opacity: 0, y: -4 }}
      transition={reduceMotion ? { duration: 0 } : springs.standard}
      style={{ display: "grid" }}
    >
      <div {...stylex.props(styles.content)}>{children}</div>
    </m.div>
  );
}
