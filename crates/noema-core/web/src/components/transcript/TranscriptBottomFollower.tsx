import { useMessageScroller } from "@/components/ui/message-scroller";
import * as React from "react";

export const ARRIVAL_SCROLL_FOLLOW_DURATION_MS = 360;

export function TranscriptBottomFollower({
  arrivalScrollKey,
  followBottomRef,
  scrollKey
}: {
  arrivalScrollKey: string;
  followBottomRef: React.MutableRefObject<boolean>;
  scrollKey: string;
}) {
  const { scrollToEnd } = useMessageScroller();

  React.useLayoutEffect(() => {
    if (followBottomRef.current) {
      scrollToEnd({ behavior: "auto" });
    }
  }, [followBottomRef, scrollKey, scrollToEnd]);

  React.useLayoutEffect(() => {
    if (!arrivalScrollKey || !followBottomRef.current) {
      return;
    }

    let animationFrame: number | null = null;
    let startedAt: number | null = null;

    const followArrival = (timestamp: number) => {
      if (!followBottomRef.current) {
        return;
      }
      if (startedAt === null) {
        startedAt = timestamp;
      }

      scrollToEnd({ behavior: "auto" });

      const elapsedMs = timestamp - startedAt;
      if (elapsedMs < ARRIVAL_SCROLL_FOLLOW_DURATION_MS) {
        animationFrame = window.requestAnimationFrame(followArrival);
      }
    };

    animationFrame = window.requestAnimationFrame(followArrival);

    return () => {
      if (animationFrame !== null) {
        window.cancelAnimationFrame(animationFrame);
      }
    };
  }, [arrivalScrollKey, followBottomRef, scrollToEnd]);

  return null;
}
