import * as React from "react";
import * as stylex from "@stylexjs/stylex";
import { useMutation, useQuery } from "@apollo/client/react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { MoreMenu } from "@astryxdesign/core/MoreMenu";
import { Switch } from "@astryxdesign/core/Switch";
import { TextArea } from "@astryxdesign/core/TextArea";
import { TextInput } from "@astryxdesign/core/TextInput";
import { VStack } from "@astryxdesign/core/VStack";
import {
  AcpAgentsDocument,
  AgentsDocument,
  AuthenticateAcpAgentDocument,
  CreateAcpAgentDocument,
  SaveAgentModelPreferenceDocument,
  TaskModelPoolsDocument,
  TestAcpAgentDocument,
  UpdateAcpAgentDocument,
  UpdateTaskModelPoolEntryDocument,
  type AcpAgentsQuery,
  type AgentsQuery,
  type SaveAgentModelPreferenceMutation,
  type SaveAgentModelPreferenceMutationVariables,
  type TaskModelPoolEntryInput,
  type TaskModelPoolsQuery,
  type UpdateTaskModelPoolEntryMutation,
  type UpdateTaskModelPoolEntryMutationVariables
} from "@/generated/graphql";
import { ModelPreferenceSelect } from "./ModelPreferenceSelect";
import { TaskModelPoolsSettings } from "./TaskModelPoolsSettings";
import type {
  ModelPreference,
  ModelPreferenceSaveInput,
  ModelProviderOption
} from "./modelPreferenceTypes";
import { agentDisplayName, selectedModelWarning } from "./agentMetadata";
import { SettingsEditDialog } from "./SettingsEditDialog";
import {
  SettingsList,
  SettingsListItem,
  SettingsLocalFeedback,
  SettingsRowActions,
  SettingsSection,
  SettingsSectionBody,
  SettingsSectionInset
} from "./SettingsPrimitives";
import { settingsStatusLabel } from "./settingsStatus";

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

export function AgentsSettingsPane() {
  const agentsResult = useQuery<AgentsQuery>(AgentsDocument, { fetchPolicy: "cache-and-network" });
  const acpAgentsResult = useQuery<AcpAgentsQuery>(AcpAgentsDocument, { fetchPolicy: "cache-and-network" });
  const poolsResult = useQuery<TaskModelPoolsQuery>(TaskModelPoolsDocument, { fetchPolicy: "cache-and-network" });
  const [savePreference, saveResult] = useMutation<SaveAgentModelPreferenceMutation, SaveAgentModelPreferenceMutationVariables>(SaveAgentModelPreferenceDocument, { refetchQueries: [{ query: AgentsDocument }], awaitRefetchQueries: true });
  const [updatePool, updatePoolResult] = useMutation<UpdateTaskModelPoolEntryMutation, UpdateTaskModelPoolEntryMutationVariables>(UpdateTaskModelPoolEntryDocument, { refetchQueries: [{ query: TaskModelPoolsDocument }], awaitRefetchQueries: true });
  const mutationOptions = { refetchQueries: [{ query: AcpAgentsDocument }], awaitRefetchQueries: true };
  const [createAcpAgent, createAcpState] = useMutation(CreateAcpAgentDocument, mutationOptions);
  const [updateAcpAgent, updateAcpState] = useMutation(UpdateAcpAgentDocument, mutationOptions);
  const [testAcpAgent, testAcpState] = useMutation(TestAcpAgentDocument, mutationOptions);
  const [authenticateAcpAgent, authenticateAcpState] = useMutation(AuthenticateAcpAgentDocument, mutationOptions);
  const agents = agentsResult.data?.agents ?? [];
  const loading = agentsResult.loading && !agentsResult.data;
  const error = agentsResult.error?.message ?? null;
  const saving = saveResult.loading;
  const saveError = saveResult.error?.message ?? null;
  const onSaveModelPreference = (input: SaveAgentModelPreferenceInput) => savePreference({ variables: { input } });
  const taskModelPoolEntries = poolsResult.data?.taskModelPools ?? [];
  const primaryAgent = agents.find((agent) => agent.isPrimary) ?? agents[0];
  const taskModelPoolModelOptions = primaryAgent?.modelOptions ?? [];
  const taskModelPoolLoading = poolsResult.loading && !poolsResult.data;
  const taskModelPoolError = poolsResult.error?.message ?? null;
  const taskModelPoolSaving = updatePoolResult.loading;
  const taskModelPoolSaveError = updatePoolResult.error?.message ?? null;
  const onUpdateTaskModelPool = (poolEntryId: string, input: TaskModelPoolEntryInput) => updatePool({ variables: { poolEntryId, input } });
  const acpAgents = acpAgentsResult.data?.acpAgents ?? [];
  const acpLoading = acpAgentsResult.loading && !acpAgentsResult.data;
  const acpError = acpAgentsResult.error?.message ?? createAcpState.error?.message ?? updateAcpState.error?.message ?? testAcpState.error?.message ?? authenticateAcpState.error?.message ?? null;
  const acpBusy = createAcpState.loading || updateAcpState.loading || testAcpState.loading || authenticateAcpState.loading;
  const onCreateAcpAgent = (input: { displayName: string; command: string; arguments: string[] }) => createAcpAgent({ variables: { input } });
  const onUpdateAcpAgent = (input: { agentId: string; expectedRevision: number; displayName: string; command: string; arguments: string[]; enabled: boolean }) => updateAcpAgent({ variables: { input } });
  const onTestAcpAgent = (input: { agentId: string; expectedRevision: number }) => testAcpAgent({ variables: { input } });
  const onAuthenticateAcpAgent = (input: { agentId: string; expectedRevision: number; methodId: string }) => authenticateAcpAgent({ variables: { input } });
  const [editingAcpAgent, setEditingAcpAgent] = React.useState<AcpAgent | "new" | null>(null);
  if (loading) {
    return <p {...stylex.props(styles.mutedText)}>Loading agents...</p>;
  }

  const taskExecutor = agents.find((agent) => agent.agentId === TASK_EXECUTOR_AGENT_ID);
  const visibleAgents = agents.filter((agent) => agent.agentId !== TASK_EXECUTOR_AGENT_ID);

  return (
    <VStack gap={4} {...stylex.props(styles.stack)}>
      <SettingsSection title="Agent models" titleId="registered-agents-title">
          {error ? (
            <SettingsSectionInset><p role="alert" {...stylex.props(styles.mutedText)}>Agent metadata could not be loaded.</p></SettingsSectionInset>
          ) : visibleAgents.length === 0 ? (
            <SettingsSectionInset><p {...stylex.props(styles.mutedText)}>No agents were found.</p></SettingsSectionInset>
          ) : (
            <SettingsList density="balanced" hasDividers>
              {visibleAgents.map((agent) => (
                <AgentRow key={agent.agentId} agent={agent} saving={saving} onSave={onSaveModelPreference} />
              ))}
            </SettingsList>
          )}
          {saveError ? <SettingsLocalFeedback>
            <p role="alert" {...stylex.props(styles.saveError)}>Noema could not save the model choice.</p>
          </SettingsLocalFeedback> : null}
      </SettingsSection>
      <SettingsSection
        title="ACP task executors"
        titleId="acp-agents-title"
        action={<Button type="button" size="sm" variant="secondary" label="Add ACP agent" onClick={() => setEditingAcpAgent("new")} />}
      >
          <SettingsSectionInset>
            <p {...stylex.props(styles.mutedText)}>Trusted local commands Noema can launch for Executor runs.</p>
          </SettingsSectionInset>
          {acpError ? <SettingsLocalFeedback><p role="alert" {...stylex.props(styles.saveError)}>{acpError}</p></SettingsLocalFeedback> : null}
          {acpLoading ? <SettingsSectionInset divided><p {...stylex.props(styles.mutedText)}>Loading ACP agents...</p></SettingsSectionInset> : acpAgents.length === 0 ? (
            <SettingsSectionInset divided><p {...stylex.props(styles.mutedText)}>No ACP executors are configured.</p></SettingsSectionInset>
          ) : (
            <SettingsSectionBody divided><SettingsList density="balanced" hasDividers>
              {acpAgents.map((agent) => (
                <SettingsListItem
                  key={agent.agentId}
                  mobileEndContentFullWidth
                  label={<HStack gap={2} vAlign="center" wrap="wrap"><span {...stylex.props(styles.rowLabel)}>{agent.displayName}</span>{!agent.enabled ? <Badge variant="neutral" label="Off" /> : null}</HStack>}
                  description={acpAgentDescription(agent)}
                  endContent={
                    <MoreMenu
                      label={`Actions for ${agent.displayName}`}
                      size="sm"
                      isDisabled={acpBusy}
                      items={[
                        ...(agent.authStatus === "REQUIRED" ? acpAuthMethods(agent) : []).map((method) => ({
                          label: `Authenticate with ${method.name || method.id}`,
                          onClick: () => void onAuthenticateAcpAgent({ agentId: agent.agentId, expectedRevision: agent.connectionRevision, methodId: method.id })
                        })),
                        { label: "Test", onClick: () => void onTestAcpAgent({ agentId: agent.agentId, expectedRevision: agent.connectionRevision }) },
                        { label: "Edit", onClick: () => setEditingAcpAgent(agent) }
                      ]}
                    />
                  }
                />
              ))}
            </SettingsList></SettingsSectionBody>
          )}
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
  const save = async () => {
    const input = { displayName: displayName.trim(), command: command.trim(), arguments: argumentsText.split("\n").map((value) => value.trim()).filter(Boolean) };
    await (existing ? onUpdate({ agentId: existing.agentId, expectedRevision: existing.connectionRevision, enabled, ...input }) : onCreate(input));
    onClose();
  };
  return (
    <SettingsEditDialog
      title={existing ? "Edit ACP agent" : "Add ACP agent"}
      open
      saving={busy}
      saveLabel={existing ? "Save" : "Add agent"}
      saveDisabled={!displayName.trim() || !command.trim()}
      error={error}
      width={540}
      onOpenChange={(open) => !open && onClose()}
      onSave={save}
    >
        <VStack gap={3}>
          <p {...stylex.props(styles.mutedText)}>Noema launches this executable directly with the exact argument array. It does not use a shell.</p>
          <TextInput hasAutoFocus isRequired label="Name" value={displayName} onChange={setDisplayName} />
          <TextInput isRequired label="Executable" value={command} placeholder="/absolute/path/to/agent" onChange={setCommand} />
          <TextArea label="Arguments" description="One argument per line" rows={4} value={argumentsText} onChange={setArgumentsText} />
          {existing ? <Switch label="Enabled for new tasks" value={enabled} onChange={setEnabled} /> : null}
        </VStack>
    </SettingsEditDialog>
  );
}

function acpAgentDescription(agent: AcpAgent): React.ReactNode {
  if (agent.lastError) return agent.lastError;
  if (!agent.enabled || agent.authStatus === "REQUIRED") return undefined;
  if (!["AUTHENTICATED", "NONE"].includes(agent.authStatus)) {
    return settingsStatusLabel(agent.authStatus);
  }
  if (agent.healthStatus !== "HEALTHY") return settingsStatusLabel(agent.healthStatus);
  return undefined;
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
        <SettingsRowActions>
          <ModelPreferenceSelect
            options={agent.modelOptions ?? []}
            preference={agent.modelPreference ?? null}
            useCase={agent.isPrimary ? "PRIMARY" : "TASK_REVIEWER"}
            saving={saving}
            ariaLabel={`Model settings for ${agentDisplayName(agent)}`}
            onSave={(input) => onSave({ agentId: agent.agentId, ...input })}
          />
        </SettingsRowActions>
      }
    />
  );
}

const styles = stylex.create({
  stack: { minWidth: 0 },
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
});
