import { useMutation, useQuery } from "@apollo/client/react";
import { Badge } from "@astryxdesign/core/Badge";
import { Button } from "@astryxdesign/core/Button";
import { HStack } from "@astryxdesign/core/HStack";
import { VStack } from "@astryxdesign/core/VStack";
import { Copy, RefreshCw, Trash2 } from "lucide-react";
import { QRCodeSVG } from "qrcode.react";
import { useEffect, useState } from "react";
import * as stylex from "@stylexjs/stylex";
import {
  ClientsDocument,
  RevokeClientDocument,
  type ClientsQuery,
  type RevokeClientMutation,
  type RevokeClientMutationVariables
} from "@/generated/graphql";
import { SettingsList, SettingsListItem, SettingsSection } from "./SettingsPrimitives";
import { startClientPairing, type ClientPairing } from "./clientPairing";
import { DeleteConfirmationDialog } from "./DeleteConnectionDialog";

type PairedClient = ClientsQuery["clients"][number];

const dateFormatter = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short"
});

export function ClientsSettingsPane() {
  const clientsResult = useQuery<ClientsQuery>(ClientsDocument, {
    fetchPolicy: "cache-and-network"
  });
  const [revokeClient, revokeResult] = useMutation<RevokeClientMutation, RevokeClientMutationVariables>(RevokeClientDocument, {
    refetchQueries: [{ query: ClientsDocument }],
    awaitRefetchQueries: true
  });

  return (
    <ClientsSettingsPaneContent
      clients={clientsResult.data?.clients ?? []}
      loading={clientsResult.loading && !clientsResult.data}
      error={clientsResult.error ? "Paired clients could not be loaded." : null}
      mutationError={revokeResult.error ? "Noema could not revoke this client." : null}
      mutationSaving={revokeResult.loading}
      onRetry={() => void clientsResult.refetch()}
      onRevoke={(clientId) => {
        revokeResult.reset();
        return revokeClient({ variables: { clientId } });
      }}
    />
  );
}

function ClientsSettingsPaneContent({
  clients,
  loading,
  error,
  mutationError,
  mutationSaving,
  onRetry,
  onRevoke
}: {
  clients: readonly PairedClient[];
  loading: boolean;
  error: string | null;
  mutationError: string | null;
  mutationSaving: boolean;
  onRetry: () => void;
  onRevoke: (clientId: string) => Promise<unknown>;
}) {
  const [pairing, setPairing] = useState<ClientPairing | null>(null);
  const [pairingError, setPairingError] = useState<string | null>(null);
  const [pairingStarting, setPairingStarting] = useState(false);
  const [remainingMs, setRemainingMs] = useState<number | null>(null);
  const [copied, setCopied] = useState(false);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [revokeTarget, setRevokeTarget] = useState<PairedClient | null>(null);

  useEffect(() => {
    if (!pairing) {
      return;
    }

    const update = () => {
      const next = Math.max(0, pairing.expiresAt - Date.now());
      setRemainingMs(next);
      if (next === 0) {
        setPairing(null);
      }
    };

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
      const nextPairing = await startClientPairing();
      setRemainingMs(Math.max(0, nextPairing.expiresAt - Date.now()));
      setPairing(nextPairing);
    } catch (error) {
      setPairingError(error instanceof Error ? error.message : "Client pairing could not be started.");
    } finally {
      setPairingStarting(false);
    }
  };

  const copyPairingLink = async () => {
    if (!pairing) return;
    setCopyError(null);
    if (!navigator.clipboard?.writeText) {
      setCopyError("Copy is unavailable here; select the link to copy it.");
      return;
    }
    try {
      await navigator.clipboard.writeText(pairing.pairingUri);
      setCopied(true);
    } catch {
      setCopyError("The link could not be copied; select it to copy manually.");
    }
  };

  const revoke = async () => {
    if (!revokeTarget) return;
    try {
      await onRevoke(revokeTarget.clientId);
      setRevokeTarget(null);
    } catch {
      // The dialog keeps the target open and renders the mutation error.
    }
  };

  const activeClients = clients.filter((client) => client.revokedAt === null);
  const revokedClients = clients.filter((client) => client.revokedAt !== null);
  const pairingActive = pairing !== null;

  return (
    <VStack gap={6}>
      <SettingsSection aria-labelledby="client-pairing-title">
        <VStack gap={2}>
          <h2 id="client-pairing-title" {...stylex.props(styles.sectionTitle)}>
            Pair a client
          </h2>
          <p {...stylex.props(styles.mutedText)}>
            Create a short-lived link for a Noema client. The link stays in this page's memory and expires after 10 minutes.
          </p>
          {pairingActive ? (
            <VStack gap={3}>
              <VStack as="figure" gap={2} hAlign="center" {...stylex.props(styles.qrFigure)}>
                <QRCodeSVG value={pairing.pairingUri} size={200} level="M" marginSize={2} bgColor="var(--color-background-surface)" fgColor="var(--color-text-primary)" title="Scan this QR code to pair a Noema client" />
                <figcaption {...stylex.props(styles.mutedText)}>
                  Scan with the Noema client
                </figcaption>
              </VStack>
              <HStack gap={2} wrap="wrap" vAlign="center" {...stylex.props(styles.linkRow)}>
                <code {...stylex.props(styles.pairingLink)}>{pairing.pairingUri}</code>
                <Button type="button" variant="secondary" size="sm" label={copied ? "Copied" : "Copy link"} icon={<Copy {...stylex.props(styles.icon)} aria-hidden="true" />} onClick={() => void copyPairingLink()} />
              </HStack>
              <p role="status" {...stylex.props(styles.mutedText)}>
                Expires in {formatRemaining(remainingMs ?? 0)}
              </p>
              {copyError ? <p role="alert" {...stylex.props(styles.errorText)}>{copyError}</p> : null}
            </VStack>
          ) : (
            <VStack gap={2}>
              {remainingMs === 0 ? (
                <p {...stylex.props(styles.mutedText)}>This pairing link expired. Start a new one to continue.</p>
              ) : null}
              {pairingError ? <p role="alert" {...stylex.props(styles.errorText)}>{pairingError}</p> : null}
              <Button type="button" label={remainingMs === 0 || pairingError ? "Retry pairing" : "Start pairing"} icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />} isLoading={pairingStarting} onClick={() => void startPairing()} />
            </VStack>
          )}
        </VStack>
      </SettingsSection>

      <SettingsSection aria-labelledby="paired-clients-title">
        <VStack gap={3}>
          <h2 id="paired-clients-title" {...stylex.props(styles.sectionTitle)}>
            Paired clients
          </h2>
          {loading ? (
            <p {...stylex.props(styles.mutedText)}>Loading paired clients...</p>
          ) : error ? (
            <VStack gap={2}>
              <p role="alert" {...stylex.props(styles.errorText)}>{error}</p>
              <Button type="button" variant="secondary" size="sm" label="Retry" icon={<RefreshCw {...stylex.props(styles.icon)} aria-hidden="true" />} onClick={onRetry} />
            </VStack>
          ) : (
            <>
              <ClientGroup
                heading="Active"
                clients={activeClients}
                mutationSaving={mutationSaving}
                onRevokeClick={setRevokeTarget}
              />
              {revokedClients.length > 0 ? (
                <ClientGroup
                  heading="Revoked"
                  clients={revokedClients}
                  mutationSaving={mutationSaving}
                  onRevokeClick={setRevokeTarget}
                />
              ) : null}
            </>
          )}
        </VStack>
      </SettingsSection>

      <DeleteConfirmationDialog
        title={revokeTarget ? `Revoke ${revokeTarget.displayName}?` : "Revoke client?"}
        message={revokeTarget?.isCurrent
          ? "This is the bearer credential used by the current request. Revoking it will end that client's access."
          : "The client will stop authenticating immediately. Past activity and its audit record are kept."}
        open={revokeTarget !== null}
        submitting={mutationSaving}
        error={revokeTarget ? mutationError : null}
        confirmLabel="Revoke client"
        onOpenChange={(open) => {
          if (!open && !mutationSaving) setRevokeTarget(null);
        }}
        onConfirm={() => void revoke()}
      />
    </VStack>
  );
}

function ClientGroup({
  heading,
  clients,
  mutationSaving,
  onRevokeClick
}: {
  heading: string;
  clients: readonly PairedClient[];
  mutationSaving: boolean;
  onRevokeClick: (client: PairedClient) => void;
}) {
  return (
    <VStack gap={2}>
      <h3 {...stylex.props(styles.groupTitle)}>{heading}</h3>
      {clients.length === 0 ? (
        <p {...stylex.props(styles.mutedText)}>No {heading.toLowerCase()} clients are paired yet.</p>
      ) : (
        <SettingsList density="compact" hasDividers>
          {clients.map((client) => (
            <SettingsListItem
              key={client.clientId}
              mobileEndContentFullWidth
              label={
                <HStack gap={2} wrap="wrap" vAlign="center">
                  <span>{client.displayName}</span>
                  {client.isCurrent ? <Badge variant="info" label="Current client" /> : null}
                </HStack>
              }
              description={
                <VStack gap={1}>
                  <span>Added {formatDate(client.createdAt)}</span>
                  {client.revokedAt ? <span>Revoked {formatDate(client.revokedAt)}</span> : null}
                </VStack>
              }
              endContent={
                client.revokedAt ? (
                  <Badge variant="neutral" label="Revoked" />
                ) : (
                  <Button type="button" variant="destructive" size="sm" label="Revoke" icon={<Trash2 {...stylex.props(styles.icon)} aria-hidden="true" />} isDisabled={mutationSaving} onClick={() => onRevokeClick(client)} />
                )
              }
            />
          ))}
        </SettingsList>
      )}
    </VStack>
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
  sectionTitle: {
    margin: "var(--spacing-0)",
    fontFamily: "var(--font-heading)",
    fontSize: 16,
    lineHeight: 1.3,
    color: "var(--foreground)"
  },
  groupTitle: {
    margin: "var(--spacing-0)",
    fontSize: 13,
    lineHeight: 1.3,
    fontWeight: 600,
    color: "var(--foreground)"
  },
  mutedText: {
    margin: "var(--spacing-0)",
    color: "var(--muted-foreground)",
    fontSize: 13,
    lineHeight: 1.5
  },
  errorText: {
    margin: "var(--spacing-0)",
    color: "var(--destructive)",
    fontSize: 13,
    lineHeight: 1.5
  },
  qrFigure: {
    margin: "var(--spacing-0)",
    color: "var(--color-text-primary)"
  },
  linkRow: {
    width: "100%",
    minWidth: 0
  },
  pairingLink: {
    flex: 1,
    minWidth: 0,
    overflowWrap: "anywhere",
    fontFamily: "var(--font-family-code)",
    fontSize: 12,
    lineHeight: 1.45,
    color: "var(--color-text-secondary)"
  },
  icon: {
    width: 16,
    height: 16
  }
});
