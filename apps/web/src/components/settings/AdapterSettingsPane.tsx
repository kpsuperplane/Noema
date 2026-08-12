import { useLazyQuery, useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { useNavigate } from "@tanstack/react-router";
import { Button } from "@astryxdesign/core/Button";
import { DropdownMenu, DropdownMenuItem } from "@astryxdesign/core/DropdownMenu";
import { HStack } from "@astryxdesign/core/HStack";
import { IconButton } from "@astryxdesign/core/IconButton";
import { RadioList, RadioListItem } from "@astryxdesign/core/RadioList";
import { Switch } from "@astryxdesign/core/Switch";
import { VStack } from "@astryxdesign/core/VStack";
import { Check, FileKey2, MoreHorizontal, Pencil, Plus, Trash2, Unplug } from "lucide-react";
import * as stylex from "@stylexjs/stylex";
import { useState } from "react";
import {
  AdapterDefinitionsDocument,
  AdapterOauthAttemptDocument,
  AdapterOauthAttemptEventsDocument,
  AdapterOauthStateDocument,
  ApproveAdapterDefinitionDocument,
  AttachAdapterOauthConnectionDocument,
  CapabilityIntegrationsDocument,
  DeleteAdapterConnectionDocument,
  DeleteAdapterOauthApplicationDocument,
  DeleteAdapterServiceDocument,
  DisconnectAdapterOauthGrantDocument,
  ImportAdapterOauthApplicationDocument,
  ReplaceAdapterOauthApplicationDocument,
  SaveAdapterOauthGrantLabelDocument,
  SetAdapterConnectionActiveDocument,
  SetupAdapterConnectionDocument,
  StartAdapterOauthSetupDocument,
  type AdapterDefinitionsQuery,
  type AdapterOauthStateQuery,
  type CapabilityIntegrationsQuery
} from "@/generated/graphql";
import { reserveExternalAuthNavigation } from "@/graphql/externalUrls";
import { ListCardButton } from "@/components/ListCardLink";
import { CapabilityManagementLayout } from "./CapabilityManagementLayout";
import {
  CapabilityIntegrationList,
  CapabilityServiceIcon,
  type CapabilityProvider
} from "./CapabilityIntegrationList";
import { DeleteConfirmationDialog, DeleteConnectionDialog, DeleteServiceDialog } from "./DeleteConnectionDialog";
import { AdapterDefinitionReviewDetails } from "@/components/capabilities/AdapterDefinitionReviewDetails";
import {
  AdapterCredentialSetupDialog,
  type AdapterCredentialSubmission
} from "@/components/capabilities/AdapterCredentialSetupDialog";
import { SettingsEditDialog } from "./SettingsEditDialog";
import { TextInput } from "@astryxdesign/core/TextInput";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";

type AdapterDefinition = AdapterDefinitionsQuery["adapterDefinitions"][number];
type Grant = AdapterOauthStateQuery["adapterOauthState"]["grants"][number];
type NextAction = NonNullable<AdapterDefinition["nextAction"]>;
type ConnectionAction = AdapterDefinition["connectionActions"][number];
type OAuthAction = NextAction | ConnectionAction;
type PendingOAuthSetup = {
  action: OAuthAction;
  additionalSemanticDigests: string[];
};
type ProviderAddFlow = {
  provider: CapabilityProvider;
  applicationId: string | null;
  selectedSemanticDigests: string[];
};

export function AdapterSettingsPane({ connectionId }: { connectionId?: string }) {
  const navigate = useNavigate();
  const definitionsResult = useQuery<AdapterDefinitionsQuery>(AdapterDefinitionsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const oauthResult = useQuery<AdapterOauthStateQuery>(AdapterOauthStateDocument, {
    fetchPolicy: "cache-and-network"
  });
  const integrationsResult = useQuery<CapabilityIntegrationsQuery>(CapabilityIntegrationsDocument, {
    variables: { kind: "API" },
    fetchPolicy: "cache-and-network"
  });
  const [approve, approval] = useMutation(ApproveAdapterDefinitionDocument);
  const [setupConnection, credentialSetupState] = useMutation(SetupAdapterConnectionDocument);
  const [importApplication, applicationImportState] = useMutation(ImportAdapterOauthApplicationDocument);
  const [replaceApplication, applicationReplaceState] = useMutation(ReplaceAdapterOauthApplicationDocument);
  const [attachGrant, attachState] = useMutation(AttachAdapterOauthConnectionDocument);
  const [startOauth, oauthStartState] = useMutation(StartAdapterOauthSetupDocument);
  const [disconnectGrant, disconnectState] = useMutation(DisconnectAdapterOauthGrantDocument);
  const [saveGrantLabel, labelState] = useMutation(SaveAdapterOauthGrantLabelDocument);
  const [deleteApplication, applicationDeleteState] = useMutation(DeleteAdapterOauthApplicationDocument);
  const [setConnectionActive, connectionState] = useMutation(SetAdapterConnectionActiveDocument);
  const [deleteConnection, deletingConnection] = useMutation(DeleteAdapterConnectionDocument);
  const [deleteService, deletingService] = useMutation(DeleteAdapterServiceDocument);
  const [loadOauthAttempt] = useLazyQuery(AdapterOauthAttemptDocument, { fetchPolicy: "network-only" });
  const [setupDefinitionDigest, setSetupDefinitionDigest] = useState<string | null>(null);
  const [applicationProfileDigest, setApplicationProfileDigest] = useState<string | null>(null);
  const [replacementApplicationId, setReplacementApplicationId] = useState<string | null>(null);
  const [attemptId, setAttemptId] = useState<string | null>(null);
  const [pendingOauthSetup, setPendingOauthSetup] = useState<PendingOAuthSetup | null>(null);
  const [renameGrants, setRenameGrants] = useState<Grant[] | null>(null);
  const [labelDraft, setLabelDraft] = useState("");
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [disconnectTarget, setDisconnectTarget] = useState<Grant[] | null>(null);
  const [accessConfirmation, setAccessConfirmation] = useState<{
    definition: AdapterDefinition;
    action: OAuthAction;
  } | null>(null);
  const [connectionChoice, setConnectionChoice] = useState<{
    definition: AdapterDefinition;
    action: ConnectionAction | null;
  } | null>(null);
  const [providerAddFlow, setProviderAddFlow] = useState<ProviderAddFlow | null>(null);
  const [deleteServiceTargetId, setDeleteServiceTargetId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    await Promise.all([
      definitionsResult.refetch(),
      oauthResult.refetch(),
      integrationsResult.refetch()
    ]);
  };

  const finishOauthAttempt = async (event: {
    status: string;
    grantId?: string | null;
    grantRevision?: number | null;
  }) => {
    const pending = pendingOauthSetup;
    let policyConnectionId: string | null = null;
    if (event.status === "completed" && event.grantId && event.grantRevision != null
      && pending && (pending.action.connectionId == null || pending.action.kind === "reconnect_account")) {
      for (const semanticDigest of [pending.action.semanticDigest, ...pending.additionalSemanticDigests]) {
        const attached = await attachGrant({ variables: { input: {
          semanticDigest,
          grantId: event.grantId,
          expectedGrantRevision: event.grantRevision,
          replacementConnectionId: semanticDigest === pending.action.semanticDigest
            ? pending.action.connectionId
            : null
        } } });
        const connection = attached.data?.attachAdapterOauthConnection.connections
          .find((item) => item.grantId === event.grantId && !item.policyConfigured);
        policyConnectionId ??= connection?.connectionId ?? null;
      }
    } else if (event.status !== "completed") {
      setError(attemptFailure(event.status));
    }
    setAttemptId(null);
    setPendingOauthSetup(null);
    await refresh();
    if (policyConnectionId) {
      void navigate({ to: "/settings/tools/apis/$connectionId", params: { connectionId: policyConnectionId } });
    }
  };

  useSubscription(AdapterOauthAttemptEventsDocument, {
    variables: { attemptId: attemptId ?? "" },
    skip: attemptId === null,
    onData: ({ data }) => {
      const event = data.data?.adapterOauthAttemptEvents;
      if (!event || event.status === "authorizing") return;
      void finishOauthAttempt(event)
        .catch(() => setError("The account changed, but Noema could not refresh this view."));
    }
  });

  const definitions = (definitionsResult.data?.adapterDefinitions ?? []).filter((item) => !item.superseded);
  const oauth = oauthResult.data?.adapterOauthState;
  const integrations = integrationsResult.data?.capabilityIntegrations ?? [];
  const apiIntegrations = integrations
    .filter((integration) => integration.connections.length > 0)
    .map((integration) => ({
      ...integration,
      name: definitions.find((definition) => definition.semanticDigest === integration.sourceRevision)?.displayName
        ?? titleize(integration.name),
      connections: integration.connections.map((connection) => {
        const grant = oauth?.grants.find((item) => item.connectionIds.includes(connection.connectionId));
        return grant ? { ...connection, name: grant.accountLabel ?? "Unlabeled account" } : connection;
      })
    }));
  const pendingDefinitions = definitions.filter((item) => !item.reviewed);
  const availableDefinitions = definitions.filter((item) => item.reviewed && item.connectionCount === 0
    && !oauth?.applications.some((application) => application.profileDigest === item.oauthProfileDigest));
  const selectedConnection = integrations.flatMap((item) => item.connections)
    .find((item) => item.connectionId === connectionId) ?? null;
  const selectedDefinition = selectedConnection
    ? definitions.find((item) => item.semanticDigest === selectedConnection.sourceRevision) ?? null
    : null;
  const selectedDescriptor = selectedDefinition?.connections
    .find((item) => item.connectionId === connectionId) ?? null;
  const selectedGrant = selectedConnection
    ? oauth?.grants.find((grant) => grant.connectionIds.includes(selectedConnection.connectionId)) ?? null
    : null;
  const setupDefinition = definitions.find((item) => item.semanticDigest === setupDefinitionDigest) ?? null;
  const applicationProfile = oauth?.profiles.find((item) => item.profileDigest === applicationProfileDigest) ?? null;
  const replacementApplication = oauth?.applications.find((item) => item.applicationId === replacementApplicationId) ?? null;
  const replacementProfile = oauth?.profiles.find((item) => item.profileDigest === replacementApplication?.profileDigest) ?? null;
  const deleteServiceTarget = integrations.find((item) => item.definitionId === deleteServiceTargetId) ?? null;
  const providers = uniqueProviders(oauth?.applications.map((application) => ({
    id: oauthProviderId(application.providerDisplayName),
    name: application.providerDisplayName
  })) ?? []);
  const loading = (definitionsResult.loading && !definitionsResult.data)
    || (oauthResult.loading && !oauthResult.data)
    || (integrationsResult.loading && !integrationsResult.data);

  if (loading) return <p {...stylex.props(styles.pageState, styles.muted)}>Loading API accounts…</p>;
  if (definitionsResult.error || oauthResult.error || integrationsResult.error || !oauth) {
    return <SettingsSection {...stylex.props(styles.pageState)}><VStack gap={2}>
      <h2 {...stylex.props(styles.sectionTitle)}>API accounts</h2>
      <p {...stylex.props(styles.muted)}>Noema could not load API accounts.</p>
      <Button type="button" variant="secondary" label="Retry" {...stylex.props(styles.fit)} onClick={() => void refresh()} />
    </VStack></SettingsSection>;
  }

  const addFlowApplications = providerAddFlow
    ? oauthApplicationsForProvider(providerAddFlow.provider, oauth)
    : [];
  const addFlowDefinitions = providerAddFlow
    ? serviceDefinitionsForProvider(
      providerAddFlow.provider,
      definitions,
      oauth,
      providerAddFlow.applicationId
    )
    : [];

  async function runAction(
    definition: AdapterDefinition,
    selectedAction?: OAuthAction,
    additionalServices: Array<{ definition: AdapterDefinition; action: ConnectionAction }> = []
  ) {
    const action = selectedAction ?? definition.nextAction;
    if (!action) return;
    setAccessConfirmation(null);
    setError(null);
    if (action.kind === "review_definition") {
      await approve({ variables: { input: { semanticDigest: action.semanticDigest } } });
      await refresh();
      return;
    }
    if (action.kind === "attach_account" && action.grantId && action.expectedGrantRevision != null) {
      const attached = await attachGrant({ variables: { input: {
        semanticDigest: action.semanticDigest,
        grantId: action.grantId,
        expectedGrantRevision: action.expectedGrantRevision,
        replacementConnectionId: action.connectionId
      } } });
      await refresh();
      const connection = attached.data?.attachAdapterOauthConnection.connections
        .find((item) => item.grantId === action.grantId && !item.policyConfigured);
      if (connection) {
        void navigate({ to: "/settings/tools/apis/$connectionId", params: { connectionId: connection.connectionId } });
      }
      return;
    }
    if (action.kind === "import_application" && definition.oauthProfileDigest) {
      setApplicationProfileDigest(definition.oauthProfileDigest);
      return;
    }
    if (action.kind === "set_up_credential") {
      setSetupDefinitionDigest(definition.semanticDigest);
      return;
    }
    if (["add_account", "add_access", "reconnect_account"].includes(action.kind)
      && action.applicationId && action.expectedApplicationRevision != null) {
      const navigation = reserveExternalAuthNavigation();
      try {
        const response = await startOauth({ variables: { input: {
          applicationId: action.applicationId,
          expectedApplicationRevision: action.expectedApplicationRevision,
          grantId: action.grantId,
          expectedGrantRevision: action.expectedGrantRevision,
          semanticDigest: action.semanticDigest,
          operationIds: action.operationIds,
          additionalServices: additionalServices.map(({ definition: additionalDefinition, action: additionalAction }) => ({
            semanticDigest: additionalDefinition.semanticDigest,
            operationIds: additionalAction.operationIds
          }))
        } } });
        const attempt = response.data?.startAdapterOauthSetup;
        if (!attempt) throw new Error("missing attempt");
        setPendingOauthSetup({
          action,
          additionalSemanticDigests: additionalServices.map(({ definition: item }) => item.semanticDigest)
        });
        setAttemptId(attempt.attemptId);
        await navigation.open(attempt.authorizationUrl);
        window.addEventListener("focus", () => {
          void loadOauthAttempt({ variables: { attemptId: attempt.attemptId } }).then((result) => {
            const recovered = result.data?.adapterOauthAttempt;
            if (recovered && recovered.status !== "authorizing") return finishOauthAttempt(recovered);
            return refresh();
          }).catch(() => setError("Noema could not recover the account authorization state."));
        }, { once: true });
      } catch {
        navigation.cancel();
        throw new Error("Account authorization could not start.");
      }
    }
  }

  function requestAction(definition: AdapterDefinition, selectedAction?: OAuthAction) {
    const action = selectedAction ?? definition.nextAction;
    if (!action) return;
    if (action.kind === "add_access") {
      setAccessConfirmation({ definition, action });
      return;
    }
    void runAction(definition, selectedAction).catch(actionError(setError));
  }

  function requestConnection(definition: AdapterDefinition) {
    if (definition.connectionActions.length === 1) {
      requestAction(definition, definition.connectionActions[0]);
      return;
    }
    setConnectionChoice({ definition, action: null });
  }

  async function continueProviderAddFlow() {
    if (!providerAddFlow) return;
    const selectedDefinitions = providerAddFlow.selectedSemanticDigests
      .map((digest) => addFlowDefinitions.find((definition) => definition.semanticDigest === digest))
      .filter((definition): definition is AdapterDefinition => definition != null);
    if (providerAddFlow.applicationId) {
      const selections = selectedDefinitions.map((definition) => ({
        definition,
        action: definition.connectionActions.find((action) =>
          action.kind === "add_account" && action.applicationId === providerAddFlow.applicationId
        )
      })).filter((selection): selection is { definition: AdapterDefinition; action: ConnectionAction } =>
        selection.action != null
      );
      const [primary, ...additional] = selections;
      if (!primary) return;
      await runAction(primary.definition, primary.action, additional);
      setProviderAddFlow(null);
      return;
    }
    const [definition] = selectedDefinitions;
    if (!definition) return;
    setProviderAddFlow(null);
    requestConnection(definition);
  }

  async function importOauthApplication(submission: AdapterCredentialSubmission) {
    if (!applicationProfile || !submission.document) return;
    const bytes = new Uint8Array(await submission.document.arrayBuffer());
    await importApplication({ variables: { input: {
      profileDigest: applicationProfile.profileDigest,
      projectLabel: null,
      clientDocumentBase64: encodeBase64(bytes)
    } } });
    setApplicationProfileDigest(null);
    await refresh();
  }

  async function importDirectCredential(submission: AdapterCredentialSubmission) {
    if (!setupDefinition) return;
    const documentBase64 = submission.document
      ? encodeBase64(new Uint8Array(await submission.document.arrayBuffer()))
      : null;
    await setupConnection({ variables: { input: {
      semanticDigest: setupDefinition.semanticDigest,
      replacementConnectionId: setupDefinition.nextAction?.connectionId,
      fieldValues: submission.fieldValues,
      documentBase64
    } } });
    setSetupDefinitionDigest(null);
    await refresh();
  }

  async function replaceOauthApplication(submission: AdapterCredentialSubmission) {
    if (!replacementApplication || !submission.document) return;
    const bytes = new Uint8Array(await submission.document.arrayBuffer());
    await replaceApplication({ variables: { input: {
      applicationId: replacementApplication.applicationId,
      expectedRevision: replacementApplication.revision,
      clientDocumentBase64: encodeBase64(bytes)
    } } });
    setReplacementApplicationId(null);
    await refresh();
  }

  const sourceActions = selectedDefinition && selectedDescriptor
    && selectedDefinition.nextAction
      && selectedDefinition.nextAction.connectionId === selectedDescriptor.connectionId
      && ["add_access", "reconnect_account"].includes(selectedDefinition.nextAction.kind) ? (
      <Button type="button" size="sm" label={actionLabel(selectedDefinition)} isLoading={oauthStartState.loading}
        onClick={() => requestAction(selectedDefinition)} />
    ) : null;

  return <>
    <CapabilityManagementLayout
      kind="API"
      connectionId={connectionId}
      defaultConnectionId={apiIntegrations[0]?.connections[0]?.connectionId}
      title="APIs"
      primaryAction={{ label: "Connect API", onClick: () => void navigate({ to: "/" }) }}
      serviceName={selectedDefinition?.displayName}
      connectionName={selectedGrant ? selectedGrant.accountLabel ?? "Unlabeled account" : undefined}
      sourceActions={sourceActions}
      list={<VStack gap={3} {...stylex.props(styles.stack)}>
        {pendingDefinitions.length > 0 ? (
          <SettingsSection aria-labelledby="api-review-title"><VStack gap={2}>
            <h2 id="api-review-title" {...stylex.props(styles.sectionTitle)}>Review before connecting</h2>
            <SettingsList density="compact" hasDividers>
              {pendingDefinitions.map((definition) => (
                <SettingsListItem key={definition.semanticDigest} label={definition.displayName}
                  description={`${humanize(definition.authenticationMode)} · ${definition.origin}`}
                  endContent={<Button type="button" size="sm" label="Review API" isLoading={approval.loading}
                    onClick={() => void runAction(definition).catch(actionError(setError))} />} />
              ))}
            </SettingsList>
          </VStack></SettingsSection>
        ) : null}
        {definitions.length === 0 ? <SettingsSection aria-labelledby="api-empty-title"><VStack gap={2}>
          <h2 id="api-empty-title" {...stylex.props(styles.sectionTitle)}>No APIs set up</h2>
          <p {...stylex.props(styles.muted)}>
            Ask Noema in Chat to add Gmail, Google Calendar, or another API. You will review access before it connects.
          </p>
          <Button type="button" size="sm" label="Open Chat" {...stylex.props(styles.fit)}
            onClick={() => void navigate({ to: "/" })} />
        </VStack></SettingsSection> : null}
        {apiIntegrations.length > 0 || providers.length > 0 ? <CapabilityIntegrationList
          integrations={apiIntegrations}
          kind="API"
          selectedConnectionId={connectionId}
          emptyMessage="No APIs connected."
          providers={providers}
          hierarchyFor={(connection, integration) => {
            const grant = oauth.grants.find((item) => item.connectionIds.includes(connection.connectionId));
            if (grant) return {
              provider: {
                id: oauthProviderId(grant.providerDisplayName),
                name: grant.providerDisplayName
              },
              account: {
                id: grant.accountId ?? grant.grantId,
                name: grant.accountLabel ?? "Unlabeled account"
              }
            };
            const definition = definitions.find((item) => item.semanticDigest === integration.sourceRevision);
            const provider = directProvider(definition, integration);
            if (definition?.authenticationMode === "none") return { provider };
            return {
              provider,
              account: {
                id: connection.connectionId,
                name: connection.name
              }
            };
          }}
          providerActions={(provider) => {
            const applications = oauthApplicationsForProvider(provider, oauth);
            const services = serviceDefinitionsForProvider(provider, definitions, oauth);
            if (applications.length === 0 && services.length === 0) return null;
            return <HStack gap={1} vAlign="center">
              {services.length > 0 ? <IconButton size="sm" variant="ghost"
                label={`Add ${provider.name} account`}
                tooltip={`Add ${provider.name} account`}
                icon={<Plus aria-hidden="true" {...stylex.props(styles.icon)} />}
                onClick={() => {
                  setError(null);
                  setProviderAddFlow({
                    provider,
                    applicationId: applications[0]?.applicationId ?? null,
                    selectedSemanticDigests: []
                  });
                }} /> : null}
              {applications.map((application) => {
                const applicationName = application.projectLabel
                  ?? `${application.providerDisplayName} OAuth client`;
                return <HStack key={application.applicationId} gap={0.5} vAlign="center">
                  <IconButton size="sm" variant="ghost"
                    label={`Replace ${applicationName}`}
                    tooltip={`Replace ${applicationName}`}
                    icon={<FileKey2 aria-hidden="true" {...stylex.props(styles.icon)} />}
                    onClick={() => setReplacementApplicationId(application.applicationId)} />
                  <IconButton size="sm" variant="destructive"
                    label={`Delete ${applicationName}`}
                    tooltip={application.grantCount > 0
                      ? `Disconnect its accounts before deleting ${applicationName}`
                      : `Delete ${applicationName}`}
                    icon={<Trash2 aria-hidden="true" {...stylex.props(styles.icon)} />}
                    isDisabled={application.grantCount > 0}
                    isLoading={applicationDeleteState.loading}
                    onClick={() => void deleteApplication({ variables: { input: {
                      applicationId: application.applicationId,
                      expectedRevision: application.revision
                    } } }).then(refresh).catch(actionError(setError))} />
                </HStack>;
              })}
            </HStack>;
          }}
          accountAction={(provider, account) => {
            const grants = oauth.grants.filter((grant) =>
              oauthProviderId(grant.providerDisplayName) === provider.id
              && (grant.accountId ?? grant.grantId) === account.id
            );
            if (grants.length === 0) return null;
            const busy = disconnectState.loading || labelState.loading;
            return <DropdownMenu
              button={{
                label: `Actions for ${account.name}`,
                icon: <MoreHorizontal aria-hidden="true" {...stylex.props(styles.icon)} />,
                isIconOnly: true,
                size: "sm",
                variant: "ghost",
                isDisabled: busy
              }}
              hasChevron={false}
              placement="below"
              menuWidth={190}
            >
              <DropdownMenuItem
                label="Rename"
                icon={<Pencil aria-hidden="true" {...stylex.props(styles.icon)} />}
                isDisabled={busy}
                onClick={() => {
                  setError(null);
                  setRenameGrants(grants);
                  setLabelDraft(grants[0]?.accountLabel ?? "");
                }}
              />
              <DropdownMenuItem
                label={<span {...stylex.props(styles.destructiveMenuContent)}>Disconnect</span>}
                icon={<Unplug aria-hidden="true" {...stylex.props(styles.icon, styles.destructiveMenuContent)} />}
                isDisabled={busy}
                onClick={() => {
                  setError(null);
                  setDisconnectTarget(grants);
                }}
              />
            </DropdownMenu>;
          }}
        /> : null}
        {availableDefinitions.length > 0 ? <SettingsSection aria-labelledby="available-api-title"><VStack gap={2}>
          <h2 id="available-api-title" {...stylex.props(styles.sectionTitle)}>Connect an API</h2>
          <SettingsList density="compact" hasDividers>
            {availableDefinitions.map((definition) => (
              <SettingsListItem key={definition.semanticDigest} label={definition.displayName}
                description={nextActionDescription(definition)}
                endContent={definition.connectionActions.length > 0 ? <Button type="button" size="sm"
                  label={definition.connectionActions.length === 1
                    ? connectionActionLabel(definition, definition.connectionActions[0], oauth)
                    : "Choose account"}
                  isLoading={oauthStartState.loading || attachState.loading}
                  onClick={() => requestConnection(definition)} /> : null} />
            ))}
          </SettingsList>
        </VStack></SettingsSection> : null}
        {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
      </VStack>}
      definitionDetails={selectedDefinition ? <VStack gap={2}>
        <AdapterDefinitionReviewDetails definition={selectedDefinition} />
        <details><summary {...stylex.props(styles.summary)}>Canonical definition</summary>
          <pre {...stylex.props(styles.manifest)}>{selectedDefinition.manifestJson}</pre></details>
      </VStack> : null}
      dangerAction={selectedConnection && selectedDescriptor ? <>
        <Switch
          label="Connection enabled"
          isLabelHidden
          value={selectedDescriptor.status !== "suspended"}
          isLoading={connectionState.loading}
          changeAction={(active) => setConnectionActive({ variables: { input: {
            connectionId: selectedDescriptor.connectionId,
            expectedConnectionRevision: selectedDescriptor.connectionRevision,
            active
          } } }).then(refresh).catch(actionError(setError))} />
        <IconButton size="sm" variant="destructive" label="Delete connection" tooltip="Delete connection"
          icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />} onClick={() => setDeleteOpen(true)} />
      </> : null}
    />
    <AdapterCredentialSetupDialog
      title="Import OAuth client"
      serviceName={applicationProfile?.displayName ?? "OAuth client"}
      setup={applicationProfile?.credentialSetup} scopes={[]} open={applicationProfile !== null}
      submitting={applicationImportState.loading} error={error}
      intro="Import this provider client document once. You can reuse the application for more accounts and compatible APIs."
      submitLabel="Import OAuth client" onOpenChange={(open) => { if (!open) setApplicationProfileDigest(null); }}
      onSubmit={importOauthApplication} />
    <AdapterCredentialSetupDialog serviceName={setupDefinition?.displayName ?? "API"}
      setup={setupDefinition?.credentialSetup} scopes={setupDefinition?.scopes ?? []}
      open={setupDefinition?.credentialSetup != null} submitting={credentialSetupState.loading} error={error}
      onOpenChange={(open) => { if (!open) setSetupDefinitionDigest(null); }} onSubmit={importDirectCredential} />
    <AdapterCredentialSetupDialog
      title={`Replace ${replacementApplication?.providerDisplayName ?? "provider"} OAuth client`}
      serviceName={replacementApplication?.providerDisplayName ?? "OAuth client"}
      setup={replacementProfile?.credentialSetup} scopes={[]} open={replacementApplication !== null}
      submitting={applicationReplaceState.loading} error={error}
      intro={replacementApplication
        ? `Replace this OAuth client document. ${replacementApplication.grantCount} grants across ${replacementApplication.accountCount} accounts will use the new credential.`
        : "Replace this OAuth client document."}
      submitLabel="Replace OAuth client" onOpenChange={(open) => { if (!open) setReplacementApplicationId(null); }}
      onSubmit={replaceOauthApplication} />
    <SettingsEditDialog title="Rename account" open={renameGrants !== null} saving={labelState.loading}
      saveLabel="Save label" error={error} onOpenChange={(open) => { if (!open) setRenameGrants(null); }}
      onSave={() => renameGrants?.[0] ? saveGrantLabel({ variables: { input: {
        grantId: renameGrants[0].grantId,
        expectedAuthorityRevision: renameGrants[0].authorityRevision,
        accountLabel: labelDraft || null
      } } }).then(async () => { setRenameGrants(null); await refresh(); }).catch(actionError(setError)) : undefined}>
      <TextInput hasAutoFocus label="Account label" value={labelDraft} onChange={setLabelDraft} />
    </SettingsEditDialog>
    <SettingsEditDialog
      title={providerAddFlow ? `Add ${providerAddFlow.provider.name} account` : "Add account"}
      open={providerAddFlow !== null}
      saving={oauthStartState.loading}
      saveLabel={providerAddFlow?.applicationId ? "Continue to sign in" : "Continue"}
      saveDisabled={providerAddFlow?.selectedSemanticDigests.length === 0}
      error={error}
      onOpenChange={(open) => { if (!open) setProviderAddFlow(null); }}
      onSave={() => continueProviderAddFlow().catch(actionError(setError))}
    >
      {providerAddFlow ? <VStack gap={3}>
        <p {...stylex.props(styles.muted)}>
          {providerAddFlow.applicationId
            ? "Choose the services this account can use. You will sign in once."
            : "Choose a service. Noema will continue with its required access method."}
        </p>
        {addFlowApplications.length > 1 ? <RadioList
          label="Provider setup"
          size="sm"
          value={providerAddFlow.applicationId ?? ""}
          onChange={(applicationId) => setProviderAddFlow({
            ...providerAddFlow,
            applicationId,
            selectedSemanticDigests: []
          })}
        >
          {addFlowApplications.map((application) => <RadioListItem
            key={application.applicationId}
            value={application.applicationId}
            label={application.projectLabel ?? `${application.providerDisplayName} client`}
          />)}
        </RadioList> : null}
        <VStack gap={1.5}>
          {addFlowDefinitions.map((definition) => {
            const selected = providerAddFlow.selectedSemanticDigests.includes(definition.semanticDigest);
            return <ListCardButton
              key={definition.semanticDigest}
              selected={selected}
              aria-pressed={selected}
              aria-label={definition.displayName}
              onClick={() => setProviderAddFlow({
                ...providerAddFlow,
                selectedSemanticDigests: providerAddFlow.applicationId
                  ? selected
                    ? providerAddFlow.selectedSemanticDigests.filter((digest) => digest !== definition.semanticDigest)
                    : [...providerAddFlow.selectedSemanticDigests, definition.semanticDigest]
                  : selected ? [] : [definition.semanticDigest]
              })}
            >
              <HStack gap={2} vAlign="center">
                <CapabilityServiceIcon kind="API" definitionId={definition.definitionId} />
                <strong {...stylex.props(styles.serviceOptionLabel)}>{definition.displayName}</strong>
                {selected ? <Check aria-hidden="true" {...stylex.props(styles.icon)} /> : null}
              </HStack>
            </ListCardButton>;
          })}
        </VStack>
      </VStack> : null}
    </SettingsEditDialog>
    <SettingsEditDialog title={accessConfirmation ? `Authorize more ${accessConfirmation.definition.displayName} tools?` : "Authorize more tools?"}
      open={accessConfirmation !== null} saving={oauthStartState.loading} saveLabel="Continue to authorize" error={error}
      onOpenChange={(open) => { if (!open) setAccessConfirmation(null); }}
      onSave={() => accessConfirmation
        ? runAction(accessConfirmation.definition, accessConfirmation.action).catch(actionError(setError))
        : undefined}>
      {accessConfirmation ? <>
        <p {...stylex.props(styles.muted)}>
          Your provider will request access for {accessConfirmation.action.operationIds.length} additional {accessConfirmation.definition.displayName} tools.
          Current access stays available if you cancel or deny consent.
        </p>
        <details>
          <summary {...stylex.props(styles.summary)}>Technical details</summary>
          <VStack gap={2} {...stylex.props(styles.technicalDetails)}>
            <strong {...stylex.props(styles.rowLabel)}>Operations</strong>
            {accessConfirmation.action.operationIds.map((operation) => <code key={operation}>{operation}</code>)}
            <strong {...stylex.props(styles.rowLabel)}>New OAuth scopes</strong>
            {accessConfirmation.action.missingScopes.map((scope) => <code key={scope}>{scope}</code>)}
            <strong {...stylex.props(styles.rowLabel)}>Dependent APIs</strong>
            {dependentApiNames(accessConfirmation.action, oauth, definitions).map((name) => <span key={name}>{name}</span>)}
          </VStack>
        </details>
      </> : null}
    </SettingsEditDialog>
    <SettingsEditDialog title={connectionChoice ? `Connect ${connectionChoice.definition.displayName}` : "Connect API"}
      open={connectionChoice !== null} saving={false}
      saveLabel={connectionChoice?.action
        ? connectionActionLabel(connectionChoice.definition, connectionChoice.action, oauth)
        : "Continue"}
      saveDisabled={connectionChoice?.action == null} error={error}
      onOpenChange={(open) => { if (!open) setConnectionChoice(null); }}
      onSave={() => {
        if (!connectionChoice?.action) return;
        const { definition, action } = connectionChoice;
        setConnectionChoice(null);
        requestAction(definition, action);
      }}>
      {connectionChoice ? <SettingsList density="compact" hasDividers>
        {connectionChoice.definition.connectionActions.map((action) => <SettingsListItem
          key={connectionActionKey(action)}
          label={connectionActionLabel(connectionChoice.definition, action, oauth)}
          description={connectionActionDescription(connectionChoice.definition, action)}
          endContent={<Button type="button" size="sm" variant="secondary"
            label={connectionChoice.action === action ? "Selected" : "Select"}
            onClick={() => setConnectionChoice({ ...connectionChoice, action })} />} />)}
      </SettingsList> : null}
    </SettingsEditDialog>
    <DeleteConnectionDialog connection={selectedConnection ? { name: selectedConnection.name, toolCount: selectedConnection.toolCount } : null}
      keepsAccount
      open={deleteOpen && selectedConnection !== null} submitting={deletingConnection.loading} error={error}
      onOpenChange={(open) => setDeleteOpen(open)} onConfirm={() => selectedConnection && void deleteConnection({ variables: { input: {
        connectionId: selectedConnection.connectionId,
        expectedConnectionRevision: Number(selectedConnection.connectionRevision)
      } } }).then(async () => { setDeleteOpen(false); await refresh(); void navigate({ to: "/settings/tools/apis" }); }).catch(actionError(setError))} />
    <DeleteConfirmationDialog
      title="Disconnect this account?"
      message="Noema will remove its tokens and disable every API that uses this authorization. OAuth client setup remains available."
      open={disconnectTarget !== null}
      submitting={disconnectState.loading}
      error={error}
      confirmLabel="Disconnect account"
      onOpenChange={(open) => { if (!open) setDisconnectTarget(null); }}
      onConfirm={() => disconnectTarget && void (async () => {
        for (const grant of disconnectTarget) {
          await disconnectGrant({ variables: { input: {
            grantId: grant.grantId,
            expectedAuthorityRevision: grant.authorityRevision
          } } });
        }
        setDisconnectTarget(null);
        await refresh();
      })().catch(actionError(setError))}
    />
    <DeleteServiceDialog service={deleteServiceTarget ? { name: deleteServiceTarget.name, connectionCount: deleteServiceTarget.connections.length } : null}
      open={deleteServiceTarget !== null} submitting={deletingService.loading} error={error}
      onOpenChange={(open) => { if (!open) setDeleteServiceTargetId(null); }}
      onConfirm={() => deleteServiceTarget && void deleteService({ variables: { input: {
        definitionId: deleteServiceTarget.definitionId,
        expectedSourceRevision: deleteServiceTarget.sourceRevision
      } } }).then(async () => { setDeleteServiceTargetId(null); await refresh(); }).catch(actionError(setError))} />
  </>;
}

function actionLabel(definition: AdapterDefinition) {
  const action = definition.nextAction?.kind;
  if (action === "review_definition") return "Review API";
  if (action === "attach_account") return `Connect ${definition.displayName}`;
  if (action === "add_access") {
    const count = definition.nextAction?.operationIds.length ?? 0;
    return `Authorize ${count} more ${count === 1 ? "tool" : "tools"}`;
  }
  if (action === "reconnect_account") return "Reconnect account";
  if (action === "add_account") return "Add account";
  if (action === "import_application") return `Set up ${definition.displayName}`;
  if (action === "set_up_credential") return `Connect ${definition.displayName}`;
  if (action === "review_connection_policy") return "Review connection policy";
  return "Continue";
}

function connectionActionLabel(
  definition: AdapterDefinition,
  action: ConnectionAction,
  oauth: AdapterOauthStateQuery["adapterOauthState"]
) {
  const grant = oauth.grants.find((item) => item.grantId === action.grantId);
  const account = grant?.accountLabel ?? (grant ? "Unlabeled account" : null);
  if (action.kind === "attach_account") return account ? `Connect ${account}` : `Connect ${definition.displayName}`;
  if (action.kind === "add_access") {
    const count = action.operationIds.length;
    const label = `Authorize ${count} more ${count === 1 ? "tool" : "tools"}`;
    return account ? `${label} for ${account}` : label;
  }
  if (action.kind === "reconnect_account") return account ? `Reconnect ${account}` : "Reconnect account";
  if (action.kind === "add_account") {
    const application = oauth.applications.find((item) => item.applicationId === action.applicationId);
    return application?.projectLabel
      ? `Add account with ${application.projectLabel}`
      : `Add ${application?.providerDisplayName ?? "provider"} account`;
  }
  if (action.kind === "import_application") return `Set up ${definition.displayName}`;
  return actionLabel(definition);
}

function connectionActionKey(action: ConnectionAction) {
  return [action.kind, action.applicationId, action.grantId].filter(Boolean).join(":");
}

function connectionActionDescription(definition: AdapterDefinition, action: ConnectionAction) {
  if (action.kind === "attach_account") return "This account already has the required access.";
  if (action.kind === "add_access") {
    return `Authorize ${action.operationIds.length} more ${definition.displayName} tools. Current access stays available.`;
  }
  if (action.kind === "reconnect_account") return "Reconnect this account before attaching the API.";
  if (action.kind === "add_account") return "Use this provider setup. No new client document is required.";
  if (action.kind === "import_application") return "Import one provider client document for reuse.";
  return definition.origin;
}

function dependentApiNames(
  action: OAuthAction,
  oauth: AdapterOauthStateQuery["adapterOauthState"],
  definitions: AdapterDefinition[]
) {
  const connectionIds = new Set(
    oauth.grants.find((grant) => grant.grantId === action.grantId)?.connectionIds ?? []
  );
  const names = new Set(definitions.flatMap((definition) => definition.connections
    .filter((connection) => connectionIds.has(connection.connectionId))
    .map(() => definition.displayName)));
  return names.size === 0 ? ["No connected APIs"] : [...names];
}

function nextActionDescription(definition: AdapterDefinition) {
  const action = definition.nextAction;
  if (!action) return definition.origin;
  if (action.kind === "attach_account") return "Use an account that already has the required access.";
  if (action.kind === "add_access") return `Authorize ${action.operationIds.length} more tools. Current access stays available.`;
  if (action.kind === "add_account") return "Use the existing provider setup. No new client document is required.";
  if (action.kind === "import_application") return "Import the provider's OAuth client document once. Noema will reuse it for compatible APIs.";
  return definition.origin;
}

function attemptFailure(status: string) {
  if (status === "denied") return "Access was not approved. Current account access did not change.";
  if (status === "expired") return "Account authorization expired. Current account access did not change.";
  if (status === "superseded") return "A newer account authorization replaced this attempt.";
  return "Account authorization failed. Current account access did not change.";
}

function actionError(setError: (message: string) => void) {
  return (caught: unknown) => setError(caught instanceof Error ? caught.message : "The API account action failed.");
}

function oauthProviderId(providerName: string) {
  return `oauth-provider:${providerName}`;
}

function oauthApplicationsForProvider(
  provider: CapabilityProvider,
  oauth: AdapterOauthStateQuery["adapterOauthState"]
) {
  return oauth.applications.filter(
    (application) => oauthProviderId(application.providerDisplayName) === provider.id
  );
}

function serviceDefinitionsForProvider(
  provider: CapabilityProvider,
  definitions: AdapterDefinition[],
  oauth: AdapterOauthStateQuery["adapterOauthState"],
  applicationId?: string | null
) {
  const applicationIds = new Set(oauthApplicationsForProvider(provider, oauth)
    .filter((application) => applicationId == null || application.applicationId === applicationId)
    .map((application) => application.applicationId));
  if (applicationIds.size > 0) {
    return definitions.filter((definition) => definition.reviewed && definition.connectionActions.some(
      (action) => action.kind === "add_account"
        && action.applicationId != null
        && applicationIds.has(action.applicationId)
    ));
  }
  return definitions.filter((definition) => definition.reviewed
    && directProviderId(definition.origin) === provider.id
    && definition.connectionActions.length > 0);
}

function directProviderId(origin: string) {
  return `provider:${origin}`;
}

function directProvider(
  definition: AdapterDefinition | undefined,
  integration: CapabilityIntegrationsQuery["capabilityIntegrations"][number]
): CapabilityProvider {
  const origin = definition?.origin;
  if (!origin) return { id: `provider:${integration.definitionId}`, name: titleize(integration.name) };
  try {
    return { id: directProviderId(origin), name: new URL(origin).hostname };
  } catch {
    return { id: directProviderId(origin), name: origin };
  }
}

function uniqueProviders(providers: CapabilityProvider[]) {
  return [...new Map(providers.map((provider) => [provider.id, provider])).values()];
}

function humanize(value: string) { return value.replaceAll("_", " "); }

function titleize(value: string) {
  return humanize(value).replace(/\b\p{L}/gu, (letter) => letter.toLocaleUpperCase());
}

function encodeBase64(bytes: Uint8Array) {
  let value = "";
  for (const byte of bytes) value += String.fromCharCode(byte);
  return btoa(value);
}

const styles = stylex.create({
  stack: { display: "grid", gap: "var(--spacing-3)" },
  pageState: { padding: "var(--spacing-4)", "@media (max-width: 760px)": { padding: "var(--spacing-3)" } },
  sectionTitle: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 16, lineHeight: 1.3, fontWeight: 600 },
  rowLabel: { color: "var(--foreground)", fontWeight: 650, overflowWrap: "anywhere" },
  muted: { margin: 0, color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 },
  error: { margin: 0, color: "var(--destructive)", fontSize: 13 },
  fit: { width: "fit-content" },
  icon: { width: 16, height: 16 },
  destructiveMenuContent: { color: "var(--destructive)" },
  serviceOptionLabel: { flexGrow: 1, minWidth: 0, fontSize: 14, lineHeight: 1.25 },
  summary: { cursor: "pointer", fontSize: 12, fontWeight: 600 },
  technicalDetails: { paddingTop: "var(--spacing-2)" },
  manifest: { maxHeight: 280, margin: "var(--spacing-2) 0 0", padding: "var(--spacing-2)", overflow: "auto", borderRadius: "var(--radius-sm)", backgroundColor: "var(--noema-surface-subtle)", fontFamily: "var(--font-mono)", fontSize: 12, whiteSpace: "pre-wrap", overflowWrap: "anywhere" }
});
