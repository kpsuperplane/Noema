import { Marker, MarkerContent } from "@/components/ui/marker";
import { cn } from "@/lib/utils";

export function ErrorMarker({
  message,
  label,
  recoverable = true,
  className
}: {
  message: string;
  label?: string;
  recoverable?: boolean;
  className?: string;
}) {
  return (
    <Marker
      role={recoverable ? "status" : "alert"}
      className={cn("w-fit max-w-[760px] text-[var(--red-700)]", className)}
    >
      <MarkerContent className="flex flex-wrap gap-x-1.5">
        {label ? <strong>{label}</strong> : null}
        <span>{message}</span>
      </MarkerContent>
    </Marker>
  );
}
