import type { ReactNode } from "react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@astryxdesign/core/Dialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
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
        height="auto"
        header={<DialogHeader title={title} onOpenChange={setOpen} />}
        content={
          <LayoutContent>
            <VStack
              as="form"
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                void Promise.resolve(onSave()).catch(() => undefined);
              }}
            >
              {children}
              {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
              <HStack gap={2} hAlign="end" wrap="wrap">
                <Button
                  type="button"
                  variant="secondary"
                  label="Cancel"
                  isDisabled={saving}
                  onClick={() => setOpen(false)}
                />
                <Button
                  type="submit"
                  label={saveLabel}
                  isDisabled={saving || saveDisabled}
                  isLoading={saving}
                />
              </HStack>
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

const styles = stylex.create({
  error: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.45
  }
});
