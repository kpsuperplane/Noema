import * as React from "react";
import { useMutation } from "@apollo/client/react";
import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import * as stylex from "@stylexjs/stylex";
import { WorkCaptureTaskDocument } from "@/generated/graphql";
import { createClientId } from "@/shared/clientId";
import type { WorkProject } from "./workTypes";
import { pwaRuntime } from "@/pwa/runtime";
import { readTaskCaptureDraft, writeTaskCaptureDraft } from "@/pwa/storage";
import { initialScheduleDraft, scheduleInput, ScheduleFields } from "./ScheduleFields";

export function CaptureTaskDialog({ open, projects, initialProjectId, onOpenChange, onCreated }: { open: boolean; projects: readonly WorkProject[]; initialProjectId?: string; onOpenChange: (open: boolean) => void; onCreated: () => void | Promise<void> }) {
  const [title, setTitle] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [projectId, setProjectId] = React.useState(initialProjectId ?? "");
  const [scheduling, setScheduling] = React.useState(false);
  const [schedule, setSchedule] = React.useState(initialScheduleDraft);
  const [capture, state] = useMutation(WorkCaptureTaskDocument);
  const pwa = React.useSyncExternalStore(
    pwaRuntime.subscribe,
    pwaRuntime.getSnapshot,
    pwaRuntime.getSnapshot
  );
  const restoredRef = React.useRef(!pwa.installed);

  React.useEffect(() => {
    if (!pwa.installed) return;
    let active = true;
    void readTaskCaptureDraft().then((draft) => {
      if (!active) return;
      if (draft) {
        setTitle(draft.title);
        setDescription(draft.description);
        setProjectId(draft.projectId);
      }
      restoredRef.current = true;
    });
    return () => {
      active = false;
    };
  }, [pwa.installed]);

  React.useEffect(() => {
    if (!pwa.installed || !restoredRef.current) return;
    const timeout = window.setTimeout(
      () => void writeTaskCaptureDraft({ title, description, projectId }),
      250
    );
    return () => window.clearTimeout(timeout);
  }, [description, projectId, pwa.installed, title]);

  React.useEffect(() => {
    if (!pwa.installed) return;
    return pwaRuntime.registerFlusher(() =>
      writeTaskCaptureDraft({ title, description, projectId })
    );
  }, [description, projectId, pwa.installed, title]);

  return (
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" width={480} aria-label="New task">
      <Layout
        height="auto"
        header={
          <DialogHeader
            title="New task"
            subtitle={scheduling ? "Runs automatically at the time you choose." : "Saved to Inbox until you queue it."}
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
                      schedule: scheduling ? scheduleInput(schedule) : null,
                      clientMutationId: createClientId()
                    }
                  }
                })
                  .then(async () => {
                    setTitle("");
                    setDescription("");
                    setProjectId("");
                    setScheduling(false);
                    setSchedule(initialScheduleDraft());
                    await writeTaskCaptureDraft({ title: "", description: "", projectId: "" });
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
              <CheckboxInput
                label="Schedule for later"
                value={scheduling}
                onChange={setScheduling}
              />
              {scheduling ? <ScheduleFields value={schedule} onChange={setSchedule} /> : null}
              {state.error ? <p role="alert" {...stylex.props(styles.error)}>{state.error.message}</p> : null}
              <HStack gap={2} justify="end" className={stylex.props(styles.actions).className}>
                <Button type="button" size="sm" variant="ghost" label="Cancel" onClick={() => onOpenChange(false)} />
                <Button type="submit" size="sm" variant="primary" label={scheduling ? "Schedule task" : "Add to Inbox"} isLoading={state.loading} isDisabled={state.loading || !pwa.canMutate || !title.trim() || (scheduling && !scheduleInput(schedule))} />
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
