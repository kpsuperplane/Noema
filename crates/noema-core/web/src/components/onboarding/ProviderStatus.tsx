import { Badge } from "@/components/ui/badge";
import type { ProviderAccountStatus } from "./types";
import { statusCopy } from "./statusCopy";

export function ProviderStatus({ status }: { status: ProviderAccountStatus }) {
  return (
    <Badge variant="outline" className="font-mono text-xs text-muted-foreground">
      Provider status: {statusCopy[status]}
    </Badge>
  );
}
