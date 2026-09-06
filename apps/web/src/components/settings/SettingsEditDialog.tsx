import { useId, type ReactNode } from "react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent, LayoutFooter } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";

export function SettingsEditDialog({
  title,
  open,
  saving,
  saveLabel,
  saveDisabled = false,
  error,
  width = 520,
  children,
  onOpenChange,
  onSave
}: {
  title: string;
  open: boolean;
  saving: boolean;
  saveLabel: string;
  saveDisabled?: boolean;
  error?: string | null;
  width?: number;
  children: ReactNode;
  onOpenChange: (open: boolean) => void;
  onSave: () => void | Promise<void>;
}) {
  const formId = useId();
  const setOpen = (next: boolean) => {
    if (!saving) onOpenChange(next);
  };

  return (
    <Dialog
      isOpen={open}
      onOpenChange={setOpen}
      purpose="form"
      width={width}
      aria-label={title}
    >
      <Layout
        xstyle={styles.layout}
        header={<DialogHeader title={title} onOpenChange={setOpen} />}
        content={
          <LayoutContent>
            <VStack
              as="form"
              id={formId}
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                if (saving || saveDisabled) return;
                void Promise.resolve(onSave()).catch(() => undefined);
              }}
            >
              {children}
              {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
            </VStack>
          </LayoutContent>
        }
        footer={
          <LayoutFooter>
            <HStack gap={2}>
              <Button
                type="button"
                variant="secondary"
                size="md"
                xstyle={styles.action}
                label="Cancel"
                isDisabled={saving}
                onClick={() => setOpen(false)}
              />
              <Button
                type="submit"
                form={formId}
                variant="primary"
                size="md"
                xstyle={styles.action}
                label={saveLabel}
                isDisabled={saving || saveDisabled}
                isLoading={saving}
              />
            </HStack>
          </LayoutFooter>
        }
      />
    </Dialog>
  );
}

const styles = stylex.create({
  layout: { height: "auto", maxHeight: "inherit", minHeight: 0 },
  action: { flex: 1, minWidth: 0 },
  error: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.45
  }
});
