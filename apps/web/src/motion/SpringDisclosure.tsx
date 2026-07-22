import type { ReactNode } from "react";
import { AnimatePresence, useIsPresent, useReducedMotion } from "motion/react";
import * as m from "motion/react-m";
import { springs } from "./springs";

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
      initial={reduceMotion ? false : { height: 0, opacity: 0, y: -4 }}
      animate={{ height: "auto", opacity: 1, y: 0 }}
      exit={{ height: 0, opacity: 0, y: -4 }}
      transition={reduceMotion ? { duration: 0 } : springs.standard}
      style={{ overflow: "hidden" }}
    >
      {children}
    </m.div>
  );
}
