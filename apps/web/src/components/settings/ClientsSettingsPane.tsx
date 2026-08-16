import { useMutation, useQuery } from "@apollo/client/react";
import { Avatar } from "@astryxdesign/core/Avatar";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { useMediaQuery } from "@astryxdesign/core/hooks";
import { HStack } from "@astryxdesign/core/HStack";
import { Layout, LayoutContent } from "@astryxdesign/core/Layout";
import { VStack } from "@astryxdesign/core/VStack";
import { ChevronRight, Copy, RefreshCw } from "lucide-react";
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
import { startClientPairing, type ClientPairing } from "./clientPairing";
import { SettingsManagementLayout } from "./SettingsManagementLayout";
import {
  SettingsList,
  SettingsListItem,
  SettingsSection,
  SettingsSectionInset,
  SettingsTechnicalDetails
} from "./SettingsPrimitives";

type PairedClient = ClientsQuery["clients"][number];

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
  const [revokeTarget, setRevokeTarget] = useState<PairedClient | null>(null);
  const clients = result.data?.clients ?? [];
  const activeClients = clients.filter((client) => client.revokedAt === null);
  const revokedClients = clients.filter((client) => client.revokedAt !== null);
  const selectedClient = clients.find((client) => client.clientId === clientId) ?? null;
  const defaultClient = activeClients.find((client) => client.isCurrent)
    ?? activeClients[0]
    ?? revokedClients[0];
  const loading = result.loading && !result.data;
  const queryError = result.error ? "Paired clients could not be loaded." : null;
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
      primaryAction={{ label: "Pair client", onClick: () => setPairingOpen(true) }}
      detailOpen={clientId !== undefined}
      detailLabel="Manage paired client"
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
          <p {...stylex.props(styles.mutedText)}>This paired client no longer exists.</p>
        </SettingsSectionInset>
      ) : undefined}
    />
    <PairClientDialog open={pairingOpen} onOpenChange={setPairingOpen} />
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
  activeClients: readonly PairedClient[];
  revokedClients: readonly PairedClient[];
  selectedClientId?: string;
  loading: boolean;
  error: string | null;
  onRetry: () => void;
}) {
  if (loading) return <p {...stylex.props(styles.mutedText)}>Loading paired clients...</p>;
  if (error && activeClients.length === 0 && revokedClients.length === 0) return (
    <SettingsSection title="Paired clients" titleId="client-load-error">
      <SettingsSectionInset>
        <p role="alert" {...stylex.props(styles.errorText)}>{error}</p>
        <Button type="button" size="sm" variant="secondary" label="Retry" onClick={onRetry} />
      </SettingsSectionInset>
    </SettingsSection>
  );
  return <VStack gap={4}>
    <BrowserAccessSettings />
    {error ? <SettingsSection title="Paired clients" titleId="client-stale-error">
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
      )) : <p {...stylex.props(styles.mutedText)}>No active clients are paired.</p>}
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

function ClientCard({ client, selected }: { client: PairedClient; selected: boolean }) {
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

function ClientDetail({ client, busy, onRevoke }: { client: PairedClient; busy: boolean; onRevoke: () => void }) {
  return <>
    <VStack gap={0.5} {...stylex.props(styles.detailHeader)}>
      <span {...stylex.props(styles.eyebrow)}>Paired client</span>
      <HStack gap={2} wrap="wrap" vAlign="center">
        <h1 {...stylex.props(styles.detailTitle)}>{client.displayName}</h1>
        {client.isCurrent ? <Badge variant="info" label="Current client" /> : null}
      </HStack>
    </VStack>
    <SettingsSection title="Access" titleId="client-access">
      <SettingsList density="balanced" hasDividers>
        <SettingsListItem label="State" description={client.revokedAt ? "Revoked" : "Active"} />
        <SettingsListItem label="Paired" description={formatDate(client.createdAt)} />
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

function PairClientDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const [pairing, setPairing] = useState<ClientPairing | null>(null);
  const [pairingError, setPairingError] = useState<string | null>(null);
  const [pairingStarting, setPairingStarting] = useState(false);
  const [remainingMs, setRemainingMs] = useState<number | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState<string | null>(null);

  useEffect(() => {
    if (!pairing) return;
    const update = () => {
      const next = Math.max(0, pairing.expiresAt - Date.now());
      setRemainingMs(next);
      if (next === 0) setPairing(null);
    };
    update();
    const timer = window.setInterval(update, 1000);
    return () => window.clearInterval(timer);
  }, [pairing]);

  const startPairing = async () => {
    setPairing(null);
    setRemainingMs(null);
    setPairingError(null);
    setCopyError(null);
    setCopied(false);
    setPairingStarting(true);
    try {
      const next = await startClientPairing();
      setPairing(next);
      setRemainingMs(Math.max(0, next.expiresAt - Date.now()));
    } catch (error) {
      setPairingError(error instanceof Error ? error.message : "Client pairing could not start.");
    } finally {
      setPairingStarting(false);
    }
  };

  const copyPairingLink = async () => {
    if (!pairing) return;
    setCopyError(null);
    if (!navigator.clipboard?.writeText) {
      setCopyError("Copy is unavailable here. Select the link to copy it.");
      return;
    }
    try {
      await navigator.clipboard.writeText(pairing.pairingUri);
      setCopied(true);
    } catch {
      setCopyError("The link could not be copied. Select it to copy manually.");
    }
  };

  return (
    <Dialog isOpen={open} onOpenChange={onOpenChange} purpose="form" width={520} aria-label="Pair a client">
      <Layout
        height="auto"
        header={<DialogHeader title="Pair a client" onOpenChange={onOpenChange} />}
        content={<LayoutContent>
          <VStack gap={3}>
            <p {...stylex.props(styles.mutedText)}>Create a short-lived link, then scan it with a Noema client.</p>
            {pairing ? <>
              <VStack as="figure" gap={2} hAlign="center" {...stylex.props(styles.qrFigure)}>
                <QRCodeSVG
                  value={pairing.pairingUri}
                  size={200}
                  level="M"
                  marginSize={2}
                  bgColor="var(--color-background-surface)"
                  fgColor="var(--color-text-primary)"
                  title="Scan this QR code to pair a Noema client"
                />
                <figcaption {...stylex.props(styles.mutedText)}>Scan with the Noema client</figcaption>
              </VStack>
              <HStack gap={2} wrap="wrap" vAlign="center" {...stylex.props(styles.linkRow)}>
                <code {...stylex.props(styles.pairingLink)}>{pairing.pairingUri}</code>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  label={copied ? "Copied" : "Copy link"}
                  icon={<Copy aria-hidden="true" size={16} />}
                  onClick={() => void copyPairingLink()}
                />
              </HStack>
              <p role="status" {...stylex.props(styles.mutedText)}>Expires in {formatRemaining(remainingMs ?? 0)}</p>
            </> : <Button
              type="button"
              label={remainingMs === 0 || pairingError ? "Retry pairing" : "Start pairing"}
              icon={<RefreshCw aria-hidden="true" size={16} />}
              isLoading={pairingStarting}
              onClick={() => void startPairing()}
            />}
            {pairingError ? <p role="alert" {...stylex.props(styles.errorText)}>{pairingError}</p> : null}
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

function formatRemaining(valueMs: number): string {
  const totalSeconds = Math.max(0, Math.ceil(valueMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
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
