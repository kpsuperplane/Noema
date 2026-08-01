import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { WorkCaptureTaskDocument } from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import type { WorkProject } from "./workTypes";

export function CaptureTaskDialog({ open, projects, initialProjectId, onOpenChange, onCreated }: { open: boolean; projects: readonly WorkProject[]; initialProjectId?: string; onOpenChange: (open: boolean) => void; onCreated: () => void | Promise<void> }) {
  const [title, setTitle] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [projectId, setProjectId] = React.useState(initialProjectId ?? "");
  const [capture, state] = useMutation(WorkCaptureTaskDocument);

  return (
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" width={480} aria-label="New task">
      <Layout
        height="auto"
        header={
          <DialogHeader
            title="New task"
            subtitle="Saved to Inbox until you queue it."
            onOpenChange={onOpenChange}
          />
        }
        content={
          <LayoutContent>
            <VStack
              as="form"
              gap={3}
              onSubmit={(event) => {
                event.preventDefault();
                void capture({
                  variables: {
                    input: {
                      workspaceId: "workspace:personal",
                      projectId: projectId || null,
                      title: title.trim(),
                      description: description.trim(),
                      clientMutationId: createClientId()
                    }
                  }
                })
                  .then(async () => {
                    setTitle("");
                    setDescription("");
                    await onCreated();
                    onOpenChange(false);
                  })
                  .catch(() => undefined);
              }}
            >
              <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                <span>Title</span>
                <input
                  data-autofocus
                  required
                  value={title}
                  {...stylex.props(styles.input)}
                  onChange={(event) => setTitle(event.currentTarget.value)}
                />
              </VStack>
              <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                <span>Description (optional)</span>
                <textarea
                  rows={3}
                  value={description}
                  {...stylex.props(styles.input, styles.textarea)}
                  onChange={(event) => setDescription(event.currentTarget.value)}
                />
              </VStack>
              <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}>
                <span>Project (optional)</span>
                <select
                  value={projectId}
                  {...stylex.props(styles.input)}
                  onChange={(event) => setProjectId(event.currentTarget.value)}
                >
                  <option value="">No project</option>
                  {projects.filter((project) => !project.archivedAt).map((project) => (
                    <option key={project.projectId} value={project.projectId}>{project.name}</option>
                  ))}
                </select>
              </VStack>
              {state.error ? <p role="alert" {...stylex.props(styles.error)}>{state.error.message}</p> : null}
              <HStack gap={2} justify="end" className={stylex.props(styles.actions).className}>
                <Button type="button" size="sm" variant="ghost" label="Cancel" onClick={() => onOpenChange(false)} />
                <Button type="submit" size="sm" variant="primary" label="Add to Inbox" isLoading={state.loading} isDisabled={state.loading || !title.trim()} />
              </HStack>
            </VStack>
          </LayoutContent>
        }
      />
    </Dialog>
  );
}

const styles = stylex.create({
  field: { fontSize: 13, fontWeight: 600 },
  input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-2)", color: "var(--foreground)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  textarea: { resize: "vertical", lineHeight: 1.5 },
  error: { margin: "var(--spacing-0)", color: "var(--destructive)", fontSize: 13 },
  actions: { paddingTop: "var(--spacing-1)" }
});
