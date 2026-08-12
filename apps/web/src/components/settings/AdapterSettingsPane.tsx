import { useLazyQuery, useMutation, useQuery, useSubscription } from "@apollo/client/react";
import { useNavigate } from "@tanstack/react-router";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { Trash2 } from "lucide-react";
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
import { CapabilityManagementLayout } from "./CapabilityManagementLayout";
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
  const [pendingAction, setPendingAction] = useState<OAuthAction | null>(null);
  const [labelGrant, setLabelGrant] = useState<Grant | null>(null);
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
    const action = pendingAction;
    let policyConnectionId: string | null = null;
    if (event.status === "completed" && event.grantId && event.grantRevision != null
      && action && action.connectionId == null) {
      const attached = await attachGrant({ variables: { input: {
        semanticDigest: action.semanticDigest,
        grantId: event.grantId,
        expectedGrantRevision: event.grantRevision
      } } });
      const connection = attached.data?.attachAdapterOauthConnection.connections
        .find((item) => item.grantId === event.grantId && !item.policyConfigured);
      policyConnectionId = connection?.connectionId ?? null;
    } else if (event.status !== "completed") {
      setError(attemptFailure(event.status));
    }
    setAttemptId(null);
    setPendingAction(null);
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
  const pendingDefinitions = definitions.filter((item) => !item.reviewed);
  const availableDefinitions = definitions.filter((item) => item.reviewed && item.connectionCount === 0);
  const selectedConnection = integrations.flatMap((item) => item.connections)
    .find((item) => item.connectionId === connectionId) ?? null;
  const selectedDefinition = selectedConnection
    ? definitions.find((item) => item.semanticDigest === selectedConnection.sourceRevision) ?? null
    : null;
  const selectedDescriptor = selectedDefinition?.connections
    .find((item) => item.connectionId === connectionId) ?? null;
  const setupDefinition = definitions.find((item) => item.semanticDigest === setupDefinitionDigest) ?? null;
  const applicationProfile = oauth?.profiles.find((item) => item.profileDigest === applicationProfileDigest) ?? null;
  const replacementApplication = oauth?.applications.find((item) => item.applicationId === replacementApplicationId) ?? null;
  const replacementProfile = oauth?.profiles.find((item) => item.profileDigest === replacementApplication?.profileDigest) ?? null;
  const deleteServiceTarget = integrations.find((item) => item.definitionId === deleteServiceTargetId) ?? null;
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

  async function runAction(definition: AdapterDefinition, selectedAction?: OAuthAction) {
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
        expectedGrantRevision: action.expectedGrantRevision
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
          operationIds: action.operationIds
        } } });
        const attempt = response.data?.startAdapterOauthSetup;
        if (!attempt) throw new Error("missing attempt");
        setPendingAction(action);
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

  const sourceActions = selectedDefinition && selectedDescriptor ? <>
    {selectedDefinition.nextAction
      && selectedDefinition.nextAction.connectionId === selectedDescriptor.connectionId
      && ["add_access", "reconnect_account"].includes(selectedDefinition.nextAction.kind) ? (
      <Button type="button" size="sm" label={actionLabel(selectedDefinition)} isLoading={oauthStartState.loading}
        onClick={() => requestAction(selectedDefinition)} />
    ) : null}
    <Button type="button" size="sm" variant="secondary"
      label={selectedDescriptor.status === "suspended" ? "Resume connection" : "Suspend connection"}
      isLoading={connectionState.loading}
      onClick={() => void setConnectionActive({ variables: { input: {
        connectionId: selectedDescriptor.connectionId,
        expectedConnectionRevision: selectedDescriptor.connectionRevision,
        active: selectedDescriptor.status === "suspended"
      } } }).then(refresh).catch(actionError(setError))} />
  </> : null;

  return <>
    <CapabilityManagementLayout
      kind="API"
      connectionId={connectionId}
      startPolicyEditing={selectedDescriptor?.policyConfigured === false}
      sourceActions={sourceActions}
      list={<VStack gap={4} {...stylex.props(styles.stack)}>
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
        {oauth.grants.length > 0 ? <AccountSections oauth={oauth} definitions={definitions}
          busy={disconnectState.loading || labelState.loading}
          onOpen={(id) => void navigate({ to: "/settings/tools/apis/$connectionId", params: { connectionId: id } })}
          onLabel={(grant) => { setLabelGrant(grant); setLabelDraft(grant.accountLabel ?? ""); }}
          onDisconnect={setDisconnectTarget}
          onChooseConnection={requestConnection} /> : null}
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
        {oauth.applications.length > 0 ? (
          <SettingsSection aria-label="Advanced OAuth client setup">
            <details>
              <summary {...stylex.props(styles.advancedSummary)}>Advanced OAuth client setup</summary>
              <VStack gap={2} {...stylex.props(styles.advancedBody)}>
                <p {...stylex.props(styles.muted)}>
                  OAuth clients hold provider setup for account authorization. Most people do not need to manage them here.
                </p>
                <HStack gap={1} wrap="wrap">
                  {oauth.profiles.map((profile) => <Button key={profile.profileDigest} type="button" size="sm" variant="secondary"
                    label={oauth.applications.some((application) => application.profileDigest === profile.profileDigest)
                      ? `Import another ${profile.displayName} OAuth client`
                      : `Import ${profile.displayName} OAuth client`}
                    onClick={() => setApplicationProfileDigest(profile.profileDigest)} />)}
                </HStack>
                <SettingsList density="compact" hasDividers>
                  {oauth.applications.map((application) => (
                    <SettingsListItem key={application.applicationId}
                      label={application.projectLabel ?? `${application.providerDisplayName} OAuth client`}
                      description={`${application.clientId} · ${application.callbackMode} · ${oauth.profiles.find((profile) => profile.profileDigest === application.profileDigest)?.credentialSetup?.redirectUri ?? "Redirect unavailable"} · ${application.accountCount} accounts`}
                      endContent={<HStack gap={1} wrap="wrap">
                        <Button type="button" size="sm" variant="secondary" label="Replace client document"
                          onClick={() => setReplacementApplicationId(application.applicationId)} />
                        <Button type="button" size="sm" variant="destructive" label="Delete OAuth client"
                          isDisabled={application.grantCount > 0} isLoading={applicationDeleteState.loading}
                          onClick={() => void deleteApplication({ variables: { input: {
                            applicationId: application.applicationId,
                            expectedRevision: application.revision
                          } } }).then(refresh).catch(actionError(setError))} />
                      </HStack>}/>
                  ))}
                </SettingsList>
              </VStack>
            </details>
          </SettingsSection>
        ) : null}
        {error ? <p role="alert" {...stylex.props(styles.error)}>{error}</p> : null}
      </VStack>}
      definitionDetails={selectedDefinition ? <VStack gap={2}>
        <AdapterDefinitionReviewDetails definition={selectedDefinition} />
        <details><summary {...stylex.props(styles.summary)}>Canonical definition</summary>
          <pre {...stylex.props(styles.manifest)}>{selectedDefinition.manifestJson}</pre></details>
      </VStack> : null}
      dangerAction={selectedConnection ? <Button type="button" variant="destructive" label="Delete connection"
        icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />} onClick={() => setDeleteOpen(true)} /> : null}
    />
    <AdapterCredentialSetupDialog
      title={`Import ${applicationProfile?.displayName ?? "provider"} OAuth client`}
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
    <SettingsEditDialog title="Label account" open={labelGrant !== null} saving={labelState.loading}
      saveLabel="Save label" error={error} onOpenChange={(open) => { if (!open) setLabelGrant(null); }}
      onSave={() => labelGrant ? saveGrantLabel({ variables: { input: {
        grantId: labelGrant.grantId,
        expectedAuthorityRevision: labelGrant.authorityRevision,
        accountLabel: labelDraft || null
      } } }).then(async () => { setLabelGrant(null); await refresh(); }).catch(actionError(setError)) : undefined}>
      <TextInput hasAutoFocus label="Account label" value={labelDraft} onChange={setLabelDraft} />
    </SettingsEditDialog>
    <SettingsEditDialog title={accessConfirmation ? `Add ${accessConfirmation.definition.displayName} access?` : "Add access?"}
      open={accessConfirmation !== null} saving={oauthStartState.loading} saveLabel="Add access" error={error}
      onOpenChange={(open) => { if (!open) setAccessConfirmation(null); }}
      onSave={() => accessConfirmation
        ? runAction(accessConfirmation.definition, accessConfirmation.action).catch(actionError(setError))
        : undefined}>
      {accessConfirmation ? <>
        <p {...stylex.props(styles.muted)}>
          This enables {accessConfirmation.action.operationIds.length} additional {accessConfirmation.definition.displayName} operations.
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

function AccountSections({ oauth, definitions, busy, onOpen, onLabel, onDisconnect, onChooseConnection }: {
  oauth: AdapterOauthStateQuery["adapterOauthState"];
  definitions: AdapterDefinition[];
  busy: boolean;
  onOpen: (connectionId: string) => void;
  onLabel: (grant: Grant) => void;
  onDisconnect: (grants: Grant[]) => void;
  onChooseConnection: (definition: AdapterDefinition) => void;
}) {
  const providers = new Map<string, Map<string, Grant[]>>();
  for (const grant of oauth.grants) {
    const accounts = providers.get(grant.providerDisplayName) ?? new Map<string, Grant[]>();
    const key = grant.accountId ?? grant.grantId;
    accounts.set(key, [...(accounts.get(key) ?? []), grant]);
    providers.set(grant.providerDisplayName, accounts);
  }
  return <SettingsSection aria-labelledby="api-accounts-title"><VStack gap={2}>
    <h2 id="api-accounts-title" {...stylex.props(styles.sectionTitle)}>Accounts</h2>
    {[...providers.entries()].map(([provider, accounts]) => <VStack key={provider} gap={1}>
      <strong {...stylex.props(styles.providerLabel)}>{provider}</strong>
      {[...accounts.values()].map((grants) => {
      const first = grants[0];
      if (!first) return null;
      const connectionIds = new Set(grants.flatMap((grant) => grant.connectionIds));
      const connections = definitions.flatMap((definition) => definition.connections.map((connection) => ({ definition, connection })))
        .filter(({ connection }) => connectionIds.has(connection.connectionId));
      return <VStack key={first.accountId ?? first.grantId} gap={1} {...stylex.props(styles.accountGroup)}>
        <HStack hAlign="between" vAlign="center" gap={2} wrap="wrap">
          <VStack gap={0}>
            <strong {...stylex.props(styles.rowLabel)}>{first.accountLabel ?? "Unlabeled account"}</strong>
            <span {...stylex.props(styles.muted)}>{connections.length} APIs</span>
          </VStack>
          <HStack gap={1} wrap="wrap">
            {grants.some((grant) => grant.status !== "active")
              ? <Badge label="Action required" variant="warning" />
              : <Badge label="Connected" variant="success" />}
            <Button type="button" size="sm" variant="secondary" label="Edit label" isDisabled={busy} onClick={() => onLabel(first)} />
            <Button type="button" size="sm" variant="destructive" label="Disconnect account" isDisabled={busy}
              onClick={() => onDisconnect(grants)} />
          </HStack>
        </HStack>
        <SettingsList density="compact" hasDividers>
          {connections.map(({ definition, connection }) => <SettingsListItem key={connection.connectionId}
            label={definition.displayName}
            description={`${connection.operationAccess.filter((item) => item.status === "available").length}/${definition.operations.length} tools available`}
            endContent={<HStack gap={1} vAlign="center" wrap="wrap"><Badge label={connectionStatus(connection.status)}
              variant={connection.status === "active" ? "success" : "warning"} />
              {definition.connectionActions.length > 0 ? <Button type="button" size="sm"
                variant="secondary" label="Add connection"
                onClick={() => onChooseConnection(definition)} /> : null}
              <Button type="button" size="sm" variant="secondary" label="Manage" onClick={() => onOpen(connection.connectionId)} /></HStack>} />)}
          {connections.length === 0 ? <SettingsListItem label="No APIs attached" /> : null}
        </SettingsList>
      </VStack>;
      })}
    </VStack>)}
  </VStack></SettingsSection>;
}

function actionLabel(definition: AdapterDefinition) {
  const action = definition.nextAction?.kind;
  if (action === "review_definition") return "Review API";
  if (action === "attach_account") return `Connect ${definition.displayName}`;
  if (action === "add_access") return `Add ${definition.displayName} access`;
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
  if (action.kind === "add_access") return account ? `Add access for ${account}` : `Add ${definition.displayName} access`;
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
    return `Approve access for ${action.operationIds.length} ${definition.displayName} operations. Current access stays available.`;
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
  if (action.kind === "add_access") return `Approve access for ${action.operationIds.length} operations. Current access stays available.`;
  if (action.kind === "add_account") return "Use the existing provider setup. No new client document is required.";
  if (action.kind === "import_application") return "Import the provider's OAuth client document once. Noema will reuse it for compatible APIs.";
  return definition.origin;
}

function connectionStatus(status: string) {
  if (status === "authentication_required") return "Reconnect";
  return humanize(status);
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

function humanize(value: string) { return value.replaceAll("_", " "); }

function encodeBase64(bytes: Uint8Array) {
  let value = "";
  for (const byte of bytes) value += String.fromCharCode(byte);
  return btoa(value);
}

const styles = stylex.create({
  stack: { display: "grid", gap: "var(--spacing-3)" },
  pageState: { padding: "var(--spacing-4)", "@media (max-width: 760px)": { padding: "var(--spacing-3)" } },
  sectionTitle: { margin: 0, fontFamily: "var(--font-heading)", fontSize: 16, lineHeight: 1.3, fontWeight: 600 },
  accountGroup: { paddingBlock: "var(--spacing-2)", borderBlockStartWidth: "var(--border-width)", borderBlockStartStyle: "solid", borderBlockStartColor: "var(--border-subtle)" },
  rowLabel: { color: "var(--foreground)", fontWeight: 650, overflowWrap: "anywhere" },
  providerLabel: { color: "var(--foreground)", fontSize: 13, fontWeight: 700 },
  muted: { margin: 0, color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 },
  error: { margin: 0, color: "var(--destructive)", fontSize: 13 },
  fit: { width: "fit-content" },
  icon: { width: 16, height: 16 },
  summary: { cursor: "pointer", fontSize: 12, fontWeight: 600 },
  advancedSummary: { cursor: "pointer", color: "var(--foreground)", fontSize: 14, fontWeight: 650 },
  advancedBody: { paddingTop: "var(--spacing-2)" },
  technicalDetails: { paddingTop: "var(--spacing-2)" },
  manifest: { maxHeight: 280, margin: "var(--spacing-2) 0 0", padding: "var(--spacing-2)", overflow: "auto", borderRadius: "var(--radius-sm)", backgroundColor: "var(--noema-surface-subtle)", fontFamily: "var(--font-mono)", fontSize: 12, whiteSpace: "pre-wrap", overflowWrap: "anywhere" }
});
