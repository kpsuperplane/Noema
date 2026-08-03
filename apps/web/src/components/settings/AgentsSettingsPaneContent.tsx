import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { CheckboxInput } from "@astryxdesign/core/CheckboxInput";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import type { AcpAgentsQuery, TaskModelPoolEntryInput, TaskModelPoolsQuery } from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { TaskModelPoolsSettings } from "./TaskModelPoolsSettings";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
import { agentDisplayName, selectedModelWarning } from "./agentMetadata";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";

export type AgentSettingsAgent = {
  agentId: string;
  displayName?: string | null;
  isPrimary?: boolean;
  modelPreference?: ModelPreference | null;
  modelOptions?: readonly ModelProviderOption[];
};

type SaveAgentModelPreferenceInput = ModelPreferenceSaveInput & {
  agentId: string;
};

const TASK_EXECUTOR_AGENT_ID = "agent:task-executor";
type AcpAgent = AcpAgentsQuery["acpAgents"][number];

export function AgentsSettingsPaneContent({
  agents,
  loading,
  error,
  saving,
  saveError,
  onSaveModelPreference,
  taskModelPoolEntries,
  taskModelPoolModelOptions,
  taskModelPoolLoading,
  taskModelPoolError,
  taskModelPoolSaving,
  taskModelPoolSaveError,
  onUpdateTaskModelPool,
  acpAgents,
  acpLoading,
  acpError,
  acpBusy,
  onCreateAcpAgent,
  onUpdateAcpAgent,
  onTestAcpAgent,
  onAuthenticateAcpAgent
}: {
  agents: readonly AgentSettingsAgent[];
  loading: boolean;
  error: string | null;
  saving: boolean;
  saveError: string | null;
  onSaveModelPreference: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
  taskModelPoolEntries: readonly TaskModelPoolsQuery["taskModelPools"][number][];
  taskModelPoolModelOptions: readonly ModelProviderOption[];
  taskModelPoolLoading: boolean;
  taskModelPoolError: string | null;
  taskModelPoolSaving: boolean;
  taskModelPoolSaveError: string | null;
  onUpdateTaskModelPool: (poolEntryId: string, input: TaskModelPoolEntryInput) => Promise<unknown>;
  acpAgents: readonly AcpAgent[];
  acpLoading: boolean;
  acpError: string | null;
  acpBusy: boolean;
  onCreateAcpAgent: (input: { displayName: string; command: string; arguments: string[] }) => Promise<unknown>;
  onUpdateAcpAgent: (input: { agentId: string; expectedRevision: number; displayName: string; command: string; arguments: string[]; enabled: boolean }) => Promise<unknown>;
  onTestAcpAgent: (input: { agentId: string; expectedRevision: number }) => Promise<unknown>;
  onAuthenticateAcpAgent: (input: { agentId: string; expectedRevision: number; methodId: string }) => Promise<unknown>;
}) {
  const [editingAcpAgent, setEditingAcpAgent] = React.useState<AcpAgent | "new" | null>(null);
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading agents...</p>;
  }

  const taskExecutor = agents.find((agent) => agent.agentId === TASK_EXECUTOR_AGENT_ID);
  const visibleAgents = agents.filter((agent) => agent.agentId !== TASK_EXECUTOR_AGENT_ID);

  return (
    <VStack gap={6} {...stylex.props(styles.stack)}>
      {saveError ? (
        <p role="alert" {...stylex.props(styles.saveError)}>
          Noema could not save the model choice.
        </p>
      ) : null}
      <SettingsSection aria-labelledby="registered-agents-title">
        <VStack gap={2}>
          <h2 id="registered-agents-title" {...stylex.props(styles.sectionTitle)}>
            Registered agents
          </h2>
          {error ? (
            <p role="alert" {...stylex.props(styles.mutedText)}>
              Agent metadata could not be loaded.
            </p>
          ) : visibleAgents.length === 0 ? (
            <p {...stylex.props(styles.mutedText)}>No agents were found.</p>
          ) : (
            <SettingsList density="balanced" hasDividers>
              {visibleAgents.map((agent) => (
                <AgentRow key={agent.agentId} agent={agent} saving={saving} onSave={onSaveModelPreference} />
              ))}
            </SettingsList>
          )}
        </VStack>
      </SettingsSection>
      <SettingsSection aria-labelledby="acp-agents-title">
        <VStack gap={2}>
          <HStack gap={2} justify="between" vAlign="center" wrap="wrap">
            <VStack gap={0.5}>
              <h2 id="acp-agents-title" {...stylex.props(styles.sectionTitle)}>ACP Work executors</h2>
              <p {...stylex.props(styles.mutedText)}>Trusted local commands Noema can launch for Executor runs.</p>
            </VStack>
            <Button type="button" size="sm" variant="secondary" label="Add ACP agent" onClick={() => setEditingAcpAgent("new")} />
          </HStack>
          {acpError ? <p role="alert" {...stylex.props(styles.saveError)}>{acpError}</p> : null}
          {acpLoading ? <p {...stylex.props(styles.mutedText)}>Loading ACP agents...</p> : acpAgents.length === 0 ? (
            <p {...stylex.props(styles.mutedText)}>No ACP executors are configured.</p>
          ) : (
            <SettingsList density="balanced" hasDividers>
              {acpAgents.map((agent) => (
                <SettingsListItem
                  key={agent.agentId}
                  mobileEndContentFullWidth
                  label={<HStack gap={2} vAlign="center" wrap="wrap"><span {...stylex.props(styles.rowLabel)}>{agent.displayName}</span><Badge variant="neutral" label="ACP" />{!agent.enabled ? <Badge variant="neutral" label="Disabled" /> : null}</HStack>}
                  description={acpAgentDescription(agent)}
                  endContent={
                    <HStack gap={1.5} wrap="wrap" justify="end">
                      {agent.authStatus === "REQUIRED" ? acpAuthMethods(agent).map((method) => (
                        <Button key={method.id} type="button" size="sm" variant="secondary" label={`Authenticate with ${method.name || method.id}`} isLoading={acpBusy} isDisabled={acpBusy} onClick={() => void onAuthenticateAcpAgent({ agentId: agent.agentId, expectedRevision: agent.connectionRevision, methodId: method.id })} />
                      )) : null}
                      <Button type="button" size="sm" variant="secondary" label="Test" isLoading={acpBusy} isDisabled={acpBusy} onClick={() => void onTestAcpAgent({ agentId: agent.agentId, expectedRevision: agent.connectionRevision })} />
                      <Button type="button" size="sm" variant="ghost" label="Edit" isDisabled={acpBusy} onClick={() => setEditingAcpAgent(agent)} />
                    </HStack>
                  }
                />
              ))}
            </SettingsList>
          )}
        </VStack>
      </SettingsSection>
      {error || !taskExecutor ? null : (
        <TaskModelPoolsSettings
          entries={taskModelPoolEntries}
          error={taskModelPoolError}
          loading={taskModelPoolLoading}
          modelOptions={taskModelPoolModelOptions}
          onUpdate={onUpdateTaskModelPool}
          saveError={taskModelPoolSaveError}
          saving={taskModelPoolSaving}
        />
      )}
      <AcpAgentDialog
        key={editingAcpAgent === "new" ? "new" : editingAcpAgent?.agentId ?? "closed"}
        agent={editingAcpAgent}
        busy={acpBusy}
        error={acpError}
        onClose={() => setEditingAcpAgent(null)}
        onCreate={onCreateAcpAgent}
        onUpdate={onUpdateAcpAgent}
      />
    </VStack>
  );
}

function AcpAgentDialog({ agent, busy, error, onClose, onCreate, onUpdate }: { agent: AcpAgent | "new" | null; busy: boolean; error: string | null; onClose: () => void; onCreate: (input: { displayName: string; command: string; arguments: string[] }) => Promise<unknown>; onUpdate: (input: { agentId: string; expectedRevision: number; displayName: string; command: string; arguments: string[]; enabled: boolean }) => Promise<unknown> }) {
  const existing = agent && agent !== "new" ? agent : null;
  const [displayName, setDisplayName] = React.useState(existing?.displayName ?? "");
  const [command, setCommand] = React.useState(existing?.command ?? "");
  const [argumentsText, setArgumentsText] = React.useState(existing?.arguments.join("\n") ?? "");
  const [enabled, setEnabled] = React.useState(existing?.enabled ?? true);
  if (!agent) return null;
  return (
    <Dialog isOpen onOpenChange={(open) => !open && onClose()} purpose="form" width={540} aria-label={existing ? "Edit ACP agent" : "Add ACP agent"}>
      <Layout height="auto" header={<DialogHeader title={existing ? "Edit ACP agent" : "Add ACP agent"} subtitle="The executable is launched directly with this exact argument array—never through a shell." onOpenChange={(open) => !open && onClose()} />} content={<LayoutContent>
        <VStack as="form" gap={3} onSubmit={(event) => { event.preventDefault(); const input = { displayName: displayName.trim(), command: command.trim(), arguments: argumentsText.split("\n").map((value) => value.trim()).filter(Boolean) }; const request = existing ? onUpdate({ agentId: existing.agentId, expectedRevision: existing.connectionRevision, enabled, ...input }) : onCreate(input); void request.then(onClose).catch(() => undefined); }}>
          <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Name</span><input data-autofocus required value={displayName} {...stylex.props(styles.input)} onChange={(event) => setDisplayName(event.currentTarget.value)} /></VStack>
          <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Executable</span><input required value={command} placeholder="/absolute/path/to/agent" {...stylex.props(styles.input)} onChange={(event) => setCommand(event.currentTarget.value)} /></VStack>
          <VStack as="label" gap={1.5} className={stylex.props(styles.field).className}><span>Arguments (one per line)</span><textarea rows={4} value={argumentsText} {...stylex.props(styles.input, styles.textarea)} onChange={(event) => setArgumentsText(event.currentTarget.value)} /></VStack>
          {existing ? <CheckboxInput label="Enabled for new tasks" value={enabled} onChange={setEnabled} /> : null}
          {error ? <p role="alert" {...stylex.props(styles.saveError)}>{error}</p> : null}
          <HStack gap={2} justify="end"><Button type="button" size="sm" variant="ghost" label="Cancel" isDisabled={busy} onClick={onClose} /><Button type="submit" size="sm" variant="primary" label={existing ? "Save" : "Add agent"} isLoading={busy} isDisabled={busy || !displayName.trim() || !command.trim()} /></HStack>
        </VStack>
      </LayoutContent>} />
    </Dialog>
  );
}

function acpAgentDescription(agent: AcpAgent): React.ReactNode {
  const implementation = [agent.implementationName, agent.implementationVersion].filter(Boolean).join(" ");
  return <span>{implementation || agent.command} · {agent.healthStatus.toLowerCase()} · auth {agent.authStatus.toLowerCase()}{agent.lastError ? ` · ${agent.lastError}` : ""}</span>;
}

function acpAuthMethods(agent: AcpAgent): Array<{ id: string; name: string | null }> {
  const capabilities = agent.capabilities as { authMethods?: Array<{ id?: string; name?: string }> };
  return (capabilities.authMethods ?? []).flatMap((method) =>
    typeof method.id === "string" ? [{ id: method.id, name: typeof method.name === "string" ? method.name : null }] : []
  );
}

function AgentRow({
  agent,
  saving,
  onSave
}: {
  agent: AgentSettingsAgent;
  saving: boolean;
  onSave: (input: SaveAgentModelPreferenceInput) => Promise<unknown>;
}) {
  const warning = selectedModelWarning(agent);
  const label = (
    <HStack gap={2} vAlign="center" wrap="wrap">
      <span {...stylex.props(styles.rowLabel)}>{agentDisplayName(agent)}</span>
      {agent.isPrimary ? <Badge variant="neutral" label="Primary" /> : null}
    </HStack>
  );
  const description = warning ? <span {...stylex.props(styles.warningText)}>{warning}</span> : undefined;

  return (
    <SettingsListItem
      mobileEndContentFullWidth
      label={label}
      description={description}
      endContent={
        <HStack wrap="wrap" gap={2} vAlign="center" {...stylex.props(styles.rowControl)}>
          <ModelPreferenceSelect
            options={agent.modelOptions ?? []}
            preference={agent.modelPreference ?? null}
            useCase={agent.isPrimary ? "PRIMARY" : "TASK_REVIEWER"}
            saving={saving}
            ariaLabel={`Model settings for ${agentDisplayName(agent)}`}
            onSave={(input) => onSave({ agentId: agent.agentId, ...input })}
          />
        </HStack>
      }
    />
  );
}

const styles = stylex.create({
  stack: { minWidth: 0 },
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  rowLabel: {
    color: "var(--foreground)",
    fontWeight: 650,
    overflowWrap: "anywhere"
  },
  mutedText: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  warningText: {
    color: "var(--warning-foreground)",
    fontSize: 12,
    lineHeight: 1.4
  },
  saveError: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.5
  },
  rowControl: {
    justifyContent: "flex-end",
    "@media (max-width: 620px)": {
      width: "100%",
      justifyContent: "flex-start",
      marginInlineStart: "0"
    }
  },
  field: { color: "var(--foreground)", fontSize: 13, fontWeight: 600 },
  input: { width: "100%", minHeight: 38, borderWidth: 1, borderStyle: "solid", borderColor: "var(--border)", borderRadius: 8, backgroundColor: "var(--background)", paddingBlock: "var(--spacing-2)", paddingInline: "var(--spacing-2)", color: "var(--foreground)", font: "inherit", ":focus-visible": { outlineWidth: 2, outlineStyle: "solid", outlineColor: "var(--ring)", outlineOffset: 2 } },
  textarea: { resize: "vertical", lineHeight: 1.5 }
});
