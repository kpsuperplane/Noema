import { useMutation, useQuery } from "@apollo/client/react";
import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import { ChevronRight, Copy } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";
import { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import * as stylex from "@stylexjs/stylex";
import { ListCardLink } from "@/components/ListCardLink";
import { Dialog, DialogHeader } from "@/components/ResponsiveDialog";
import {
  ClientsDocument,
  RevokeClientDocument,
  type ClientsQuery,
  type RevokeClientMutation,
  type RevokeClientMutationVariables
} from "@/generated/graphql";
import { DeleteConfirmationDialog } from "./DeleteConnectionDialog";
import { BrowserAccessSettings } from "./BrowserAccessSettings";
import { SettingsManagementLayout } from "./SettingsManagementLayout";
import {
  SettingsList,
  SettingsListItem,
  SettingsSection,
  SettingsSectionInset,
  SettingsTechnicalDetails
} from "./SettingsPrimitives";

type ClientRecord = ClientsQuery["clients"][number];

const dateFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short"
});

export function ClientsSettingsPane({ clientId }: { clientId?: string }) {
  const navigate = useNavigate();
  const desktop = useMediaQuery("(min-width: 980px)");
  const result = useQuery<ClientsQuery>(ClientsDocument, { fetchPolicy: "cache-and-network" });
  const [revokeClient, revokeResult] = useMutation<RevokeClientMutation, RevokeClientMutationVariables>(
    RevokeClientDocument,
    { refetchQueries: [{ query: ClientsDocument }], awaitRefetchQueries: true }
  );
  const [pairingOpen, setPairingOpen] = useState(false);
  const [revokeTarget, setRevokeTarget] = useState<ClientRecord | null>(null);
  const clients = result.data?.clients ?? [];
  const activeClients = clients.filter((client) => client.revokedAt === null);
  const revokedClients = clients.filter((client) => client.revokedAt !== null);
  const selectedClient = clients.find((client) => client.clientId === clientId) ?? null;
  const defaultClient = activeClients.find((client) => client.isCurrent)
    ?? activeClients[0]
    ?? revokedClients[0];
  const loading = result.loading && !result.data;
  const queryError = result.error ? "Connected clients could not be loaded." : null;
  const mutationError = revokeResult.error ? "Noema could not revoke this client." : null;

  useEffect(() => {
    if (!desktop || clientId || !defaultClient) return;
    void navigate({
      to: "/settings/system/clients/$clientId",
      params: { clientId: defaultClient.clientId },
      replace: true
    });
  }, [clientId, defaultClient, desktop, navigate]);

  useEffect(() => {
    if (loading || !clientId || selectedClient) return;
    void navigate({ to: "/settings/system/clients", replace: true });
  }, [clientId, loading, navigate, selectedClient]);

  const revoke = async () => {
    if (!revokeTarget) return;
    try {
      await revokeClient({ variables: { clientId: revokeTarget.clientId } });
      setRevokeTarget(null);
      void navigate({ to: "/settings/system/clients" });
    } catch {
      // Keep the dialog open so the local error can be retried.
    }
  };

  return <>
    <SettingsManagementLayout
      title="Clients"
      primaryAction={{ label: "Connect client", onClick: () => setPairingOpen(true) }}
      detailOpen={clientId !== undefined}
      detailLabel="Manage connected client"
      onDetailOpenChange={(open) => {
        if (!open) void navigate({ to: "/settings/system/clients" });
      }}
      list={
        <ClientList
          activeClients={activeClients}
          revokedClients={revokedClients}
          selectedClientId={clientId}
          loading={loading}
          error={queryError}
          onRetry={() => void result.refetch()}
        />
      }
      detail={selectedClient ? (
        <ClientDetail
          client={selectedClient}
          busy={revokeResult.loading}
          onRevoke={() => {
            revokeResult.reset();
            setRevokeTarget(selectedClient);
          }}
        />
      ) : clientId ? (
        <SettingsSectionInset>
          <p {...stylex.props(styles.mutedText)}>This connected client no longer exists.</p>
        </SettingsSectionInset>
      ) : undefined}
    />
    <ConnectClientDialog open={pairingOpen} onOpenChange={setPairingOpen} />
    <DeleteConfirmationDialog
      title={revokeTarget ? `Revoke ${revokeTarget.displayName}?` : "Revoke client?"}
      message={revokeTarget?.isCurrent
        ? "This is the bearer credential used by the current request. Revoking it ends this client's access."
        : "The client stops authenticating immediately. Past activity and its audit record remain."}
      open={revokeTarget !== null}
      submitting={revokeResult.loading}
      error={revokeTarget ? mutationError : null}
      confirmLabel="Revoke client"
      onOpenChange={(open) => {
        if (!open && !revokeResult.loading) setRevokeTarget(null);
      }}
      onConfirm={() => void revoke()}
    />
  </>;
}

function ClientList({
  activeClients,
  revokedClients,
  selectedClientId,
  loading,
  error,
  onRetry
}: {
  activeClients: readonly ClientRecord[];
  revokedClients: readonly ClientRecord[];
  selectedClientId?: string;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  if (loading) return <p {...stylex.props(styles.mutedText)}>Loading connected clients...</p>;
  if (error && activeClients.length === 0 && revokedClients.length === 0) return (
    <SettingsSection title="Connected clients" titleId="client-load-error">
      <SettingsSectionInset>
        <p role="alert" {...stylex.props(styles.errorText)}>{error}</p>
        <Button type="button" size="sm" variant="secondary" label="Retry" onClick={onRetry} />
      </SettingsSectionInset>
    </SettingsSection>
  );
  return <VStack gap={4}>
    <BrowserAccessSettings />
    {error ? <SettingsSection title="Connected clients" titleId="client-stale-error">
      <SettingsSectionInset>
        <HStack gap={2} wrap="wrap" vAlign="center">
          <p role="alert" {...stylex.props(styles.errorText)}>{error}</p>
          <Button type="button" size="sm" variant="secondary" label="Retry" onClick={onRetry} />
        </HStack>
      </SettingsSectionInset>
    </SettingsSection> : null}
    <VStack as="section" gap={1.5} aria-labelledby="active-clients-title">
      <h2 id="active-clients-title" {...stylex.props(styles.groupTitle)}>Active</h2>
      {activeClients.length > 0 ? activeClients.map((client) => (
        <ClientCard key={client.clientId} client={client} selected={selectedClientId === client.clientId} />
      )) : <p {...stylex.props(styles.mutedText)}>No active clients are connected.</p>}
    </VStack>
    {revokedClients.length > 0 ? (
      <details open={revokedClients.some((client) => client.clientId === selectedClientId)}>
        <summary {...stylex.props(styles.revokedSummary)}>Revoked clients</summary>
        <VStack gap={1.5} {...stylex.props(styles.revokedList)}>
          {revokedClients.map((client) => (
            <ClientCard key={client.clientId} client={client} selected={selectedClientId === client.clientId} />
          ))}
        </VStack>
      </details>
    ) : null}
  </VStack>;
}

function ClientCard({ client, selected }: { client: ClientRecord; selected: boolean }) {
  return (
    <ListCardLink
      to="/settings/system/clients/$clientId"
      params={{ clientId: client.clientId }}
      selected={selected}
      aria-current={selected ? "page" : undefined}
      xstyle={styles.clientCard}
    >
      <Avatar name={client.displayName} size="sm" tooltip={false} />
      <VStack gap={0.5} {...stylex.props(styles.cardCopy)}>
        <HStack gap={1} wrap="wrap" vAlign="center">
          <strong {...stylex.props(styles.cardTitle)}>{client.displayName}</strong>
          {client.isCurrent ? <Badge variant="info" label="Current client" /> : null}
        </HStack>
        <span {...stylex.props(styles.cardMeta)}>Added {formatDate(client.createdAt)}</span>
      </VStack>
      <ChevronRight aria-hidden="true" {...stylex.props(styles.chevron)} />
    </ListCardLink>
  );
}

function ClientDetail({ client, busy, onRevoke }: { client: ClientRecord; busy: boolean; onRevoke: () => void }) {
  return <>
    <VStack gap={0.5} {...stylex.props(styles.detailHeader)}>
      <span {...stylex.props(styles.eyebrow)}>Connected client</span>
      <HStack gap={2} wrap="wrap" vAlign="center">
        <h1 {...stylex.props(styles.detailTitle)}>{client.displayName}</h1>
        {client.isCurrent ? <Badge variant="info" label="Current client" /> : null}
      </HStack>
    </VStack>
    <SettingsSection title="Access" titleId="client-access">
      <SettingsList density="balanced" hasDividers>
        <SettingsListItem label="State" description={client.revokedAt ? "Revoked" : "Active"} />
        <SettingsListItem label="Connected" description={formatDate(client.createdAt)} />
      </SettingsList>
      <SettingsTechnicalDetails>
        <SettingsList density="compact">
          <SettingsListItem label="Client ID" description={client.clientId} />
          {client.revokedAt ? <SettingsListItem label="Revoked" description={formatDate(client.revokedAt)} /> : null}
        </SettingsList>
      </SettingsTechnicalDetails>
    </SettingsSection>
    {!client.revokedAt ? (
      <SettingsSection
        title="Lifecycle"
        titleId="client-lifecycle"
        action={<Button type="button" size="sm" variant="destructive" label="Revoke client" isDisabled={busy} onClick={onRevoke} />}
      >
        <SettingsSectionInset>
          <p {...stylex.props(styles.mutedText)}>Revoking this client stops new authenticated requests. Its audit history remains.</p>
        </SettingsSectionInset>
      </SettingsSection>
    ) : null}
  </>;
}

function ConnectClientDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [connectionUri, setConnectionUri] = useState<string | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState<string | null>(null);

  const createConnectionLink = () => {
    setConnectionUri(null);
    setConnectionError(null);
    setCopyError(null);
    setCopied(false);
    try {
      setConnectionUri(clientConnectionUri());
    } catch (error) {
      setConnectionError(error instanceof Error ? error.message : "Noema could not create the connection link.");
    }
  };

  const copyConnectionLink = async () => {
    if (!connectionUri) return;
    setCopyError(null);
    if (!navigator.clipboard?.writeText) {
      setCopyError("Copy is unavailable here. Select the link to copy it.");
      return;
    }
    try {
      await navigator.clipboard.writeText(connectionUri);
      setCopied(true);
    } catch {
      setCopyError("The link could not be copied. Select it to copy manually.");
    }
  };

  return (
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" width={520} aria-label="Connect a client">
      <Layout
        height="auto"
        header={<DialogHeader title="Connect a client" onOpenChange={onOpenChange} />}
        content={<LayoutContent>
          <VStack gap={3}>
            <p {...stylex.props(styles.mutedText)}>Create a link, then scan it with a Noema client. The client will request passkey authorization.</p>
            {connectionUri ? <>
              <VStack as="figure" gap={2} hAlign="center" {...stylex.props(styles.qrFigure)}>
                <QRCodeSVG
                  value={connectionUri}
                  size={200}
                  level="M"
                  marginSize={2}
                  bgColor="var(--color-background-surface)"
                  fgColor="var(--color-text-primary)"
                  title="Scan this QR code to connect a Noema client"
                />
                <figcaption {...stylex.props(styles.mutedText)}>Scan with the Noema client</figcaption>
              </VStack>
              <HStack gap={2} wrap="wrap" vAlign="center" {...stylex.props(styles.linkRow)}>
                <code {...stylex.props(styles.pairingLink)}>{connectionUri}</code>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  label={copied ? "Copied" : "Copy link"}
                  icon={<Copy aria-hidden="true" size={16} />}
                  onClick={() => void copyConnectionLink()}
                />
              </HStack>
            </> : <Button
              type="button"
              label={connectionError ? "Retry" : "Create connection link"}
              onClick={createConnectionLink}
            />}
            {connectionError ? <p role="alert" {...stylex.props(styles.errorText)}>{connectionError}</p> : null}
            {copyError ? <p role="alert" {...stylex.props(styles.errorText)}>{copyError}</p> : null}
          </VStack>
        </LayoutContent>}
      />
    </Dialog>
  );
}

function formatDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "Unknown date" : dateFormatter.format(date);
}

function clientConnectionUri(): string {
  const origin = new URL(window.location.origin);
  if (origin.protocol !== "https:" || origin.hostname === "localhost" || origin.hostname.endsWith(".localhost")) {
    throw new Error("Client connections require the public HTTPS Noema origin.");
  }
  const connection = new URL("noema://connect");
  connection.searchParams.set("origin", origin.origin);
  return connection.toString();
}

const styles = stylex.create({
  groupTitle: {
    margin: "var(--spacing-0) var(--spacing-1)",
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 15,
    fontWeight: 700,
    lineHeight: 1.3
  },
  clientCard: {
    gridTemplateColumns: "auto minmax(0, 1fr) auto",
    alignItems: "center",
    gap: "var(--spacing-2)",
    minHeight: "var(--spacing-12)"
  },
  cardCopy: { minWidth: 0 },
  cardTitle: { color: "var(--foreground)", fontSize: 13, fontWeight: 650, overflowWrap: "anywhere" },
  cardMeta: { color: "var(--muted-foreground)", fontSize: 12 },
  chevron: { width: "var(--spacing-4)", height: "var(--spacing-4)", color: "var(--muted-foreground)" },
  revokedSummary: { color: "var(--foreground)", fontSize: 14, fontWeight: 650 },
  revokedList: { paddingBlockStart: "var(--spacing-2)" },
  detailHeader: { minWidth: 0, paddingBlockEnd: "var(--spacing-3)" },
  eyebrow: { color: "var(--muted-foreground)", fontSize: 12, lineHeight: 1.2 },
  detailTitle: {
    minWidth: 0,
    margin: 0,
    color: "var(--foreground)",
    fontFamily: "var(--font-heading)",
    fontSize: 18,
    fontWeight: 650,
    lineHeight: 1.25,
    overflowWrap: "anywhere"
  },
  mutedText: { margin: 0, color: "var(--muted-foreground)", fontSize: 13, lineHeight: 1.5 },
  errorText: { margin: 0, color: "var(--destructive)", fontSize: 13, lineHeight: 1.5 },
  qrFigure: { margin: 0, color: "var(--color-text-primary)" },
  linkRow: { width: "100%", minWidth: 0 },
  pairingLink: {
    flex: 1,
    minWidth: 0,
    overflowWrap: "anywhere",
    fontFamily: "var(--font-family-code)",
    fontSize: 12,
    lineHeight: 1.45,
    color: "var(--color-text-secondary)"
  }
});
