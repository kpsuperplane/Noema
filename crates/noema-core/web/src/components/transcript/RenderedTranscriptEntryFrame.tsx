import * as React from "react";

export function RenderedTranscriptEntryFrame({
  animateArrival,
  children
}: {
  animateArrival: boolean;
  children: React.ReactNode;
}) {
  if (!animateArrival) {
    return <>{children}</>;
  }

  return (
    <div data-slot="message-arrival-content" className="w-full max-w-[760px] min-w-0">
      <div data-slot="message-arrival-inner" className="min-h-0">
        {children}
      </div>
    </div>
  );
}
