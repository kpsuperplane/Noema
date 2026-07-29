import * as stylex from "@stylexjs/stylex";

const glimmer = stylex.keyframes({
  from: { backgroundPosition: "130% 0" },
  to: { backgroundPosition: "-30% 0" }
});

export const skeletonGlimmerStyles = stylex.create({
  animated: {
    backgroundImage:
      "linear-gradient(105deg, var(--skeleton-glimmer-base) 0%, var(--skeleton-glimmer-base) 34%, var(--skeleton-glimmer-flare) 50%, var(--skeleton-glimmer-base) 66%, var(--skeleton-glimmer-base) 100%)",
    backgroundSize: "230% 100%",
    backgroundPosition: "130% 0",
    animationDuration: "2s",
    animationIterationCount: "infinite",
    animationName: glimmer,
    animationTimingFunction: "cubic-bezier(0.45, 0, 0.2, 1)",
    "@media (prefers-reduced-motion: reduce)": {
      animationName: "none",
      backgroundImage: "none"
    }
  }
});
