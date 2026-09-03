import { IconButton } from "@astryxdesign/core/IconButton";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import { InfoIcon } from "lucide-react";
import { useState } from "react";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { toolDetailRows, toolMarkerSummary } from "./markerModel";
import type { ToolMarkerGroup } from "./renderModel";
import { ToolDetailRow } from "./ToolDetailRow";

export function ToolTechnicalRecord({ marker }: { marker: ToolMarkerGroup }) {
  const [open, setOpen] = useState(false);
  const rows = toolDetailRows(marker);
  if (rows.length === 0) {
    return null;
  }

  return (
    <>
      <IconButton
        type="button"
        size="sm"
        variant="ghost"
        label={`View technical record for ${toolMarkerSummary(marker)}`}
        tooltip="Technical record"
        icon={<InfoIcon aria-hidden="true" size={14} />}
        onClick={() => setOpen(true)}
      />
      <Dialog
        isOpen={open}
        onOpenChange={setOpen}
        purpose="info"
        width={680}
        maxHeight="min(760px, calc(100dvh - var(--spacing-8)))"
        aria-label="Technical record"
      >
        <Layout
          height="auto"
          header={<DialogHeader title="Technical record" subtitle={toolMarkerSummary(marker)} onOpenChange={setOpen} />}
          content={
            <LayoutContent>
              <VStack as="dl" gap={1}>
                {rows.map((row, index) => (
                  <ToolDetailRow key={`${row.label}:${index}`} label={row.label} value={row.value} technical />
                ))}
              </VStack>
            </LayoutContent>
          }
        />
      </Dialog>
    </>
  );
}
