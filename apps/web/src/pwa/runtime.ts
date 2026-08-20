import {
  ApolloLink,
  Observable,
  type ApolloClient,
  type NormalizedCacheObject
} from "@apollo/client";
import type { DocumentNode } from "graphql";
import {
  AdapterManagementRootDocument,
  AcpAgentsDocument,
  AgentsSettingsRootDocument,
  CapabilityConnectionDocument,
  ChatBootDocument,
  ClientsDocument,
  ConversationTranscriptPageDocument,
  LocalModelsSettingsDocument,
  LocalStatusDocument,
  McpManagementRootDocument,
  MemoryPageDocument,
  MemorySettingsDocument,
  MemoryTreeDocument,
  PendingHumanInterventionsDocument,
  PrimaryConversationDocument,
  PrivacySettingsDocument,
  ProvidersSettingsRootDocument,
  UsageSettingsRootDocument,
  WebSettingsRootDocument,
  TasksOverviewDocument,
  TasksProjectsDocument,
  TasksTaskDetailDocument,
  TasksTaskEditFieldsDocument,
  TasksTaskReferenceDocument
} from "@/generated/graphql";
import {
  isInstalledPwa,
  erasePwaPrivateStorage,
  markAuthenticated,
  requestPersistentStorage,
  saveDurableSnapshot,
  type CachedQuery,
  type DurableSnapshot
} from "./storage";

export type PwaRecoveryState = "offline" | "checking" | "reconciling" | "online" | "auth_required";

export type PwaRuntimeSnapshot = {
  installed: boolean;
  state: PwaRecoveryState;
  lastSync: number | null;
  updating: boolean;
  canMutate: boolean;
};

const mutableDocuments: ReadonlyMap<string, DocumentNode> = new Map<string, DocumentNode>([
  ["AdapterManagementRoot", AdapterManagementRootDocument],
  ["AcpAgents", AcpAgentsDocument],
  ["AgentsSettingsRoot", AgentsSettingsRootDocument],
  ["CapabilityConnection", CapabilityConnectionDocument],
  ["ChatBoot", ChatBootDocument],
  ["Clients", ClientsDocument],
  ["ConversationTranscriptPage", ConversationTranscriptPageDocument],
  ["LocalModelsSettings", LocalModelsSettingsDocument],
  ["LocalStatus", LocalStatusDocument],
  ["McpManagementRoot", McpManagementRootDocument],
  ["MemoryPage", MemoryPageDocument],
  ["MemorySettings", MemorySettingsDocument],
  ["MemoryTree", MemoryTreeDocument],
  ["PendingHumanInterventions", PendingHumanInterventionsDocument],
  ["PrimaryConversation", PrimaryConversationDocument],
  ["PrivacySettings", PrivacySettingsDocument],
  ["ProvidersSettingsRoot", ProvidersSettingsRootDocument],
  ["UsageSettingsRoot", UsageSettingsRootDocument],
  ["WebSettingsRoot", WebSettingsRootDocument],
  ["TasksOverview", TasksOverviewDocument],
  ["TasksProjects", TasksProjectsDocument],
  ["TasksTaskDetail", TasksTaskDetailDocument],
  ["TasksTaskEditFields", TasksTaskEditFieldsDocument],
  ["TasksTaskReference", TasksTaskReferenceDocument]
]);

class PwaRuntime {
  private snapshot: PwaRuntimeSnapshot = {
    installed: isInstalledPwa(),
    state: "online",
    lastSync: null,
    updating: false,
    canMutate: true
  };

  private generation = 0;
  private recentQueries: CachedQuery[] = [];
  private readonly listeners = new Set<() => void>();
  private readonly criticalOperations = new Set<string>();
  private readonly safePointWaiters = new Set<() => void>();
  private readonly flushers = new Set<() => Promise<void>>();
  private client: ApolloClient | null = null;
  private baselineCache: NormalizedCacheObject | null = null;
  private recoveryPromise: Promise<void> | null = null;
  private applicationUpdatePromise: Promise<void> | null = null;
  private persistenceTimer: number | null = null;
  private listenersInstalled = false;
  private erasing = false;

  initialize(snapshot: DurableSnapshot | null) {
    if (!this.snapshot.installed) return;
    this.generation = snapshot?.generation ?? 0;
    this.recentQueries = snapshot?.recentQueries ?? [];
    this.update({
      state: navigator.onLine ? "checking" : "offline",
      lastSync: snapshot?.lastSync ?? null,
      canMutate: false
    });
  }

  attach(client: ApolloClient) {
    this.client = client;
    if (!this.snapshot.installed || this.listenersInstalled) return;
    this.listenersInstalled = true;
    window.addEventListener("online", () => void this.resume());
    window.addEventListener("offline", () => this.goOffline());
    document.addEventListener("visibilitychange", () => {
      if (document.visibilityState === "visible") void this.resume();
      if (document.visibilityState === "hidden") void this.persistNow();
    });
  }

  getSnapshot = () => this.snapshot;

  subscribe = (listener: () => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  requestGeneration() {
    return this.generation;
  }

  async authenticated() {
    if (!this.snapshot.installed) {
      this.update({ state: "online", canMutate: true });
      return;
    }
    if (this.recoveryPromise) return this.recoveryPromise;
    this.recoveryPromise = (async () => {
      await markAuthenticated();
      void requestPersistentStorage();
      await this.prepareRecovery();
    })().finally(() => {
      this.recoveryPromise = null;
    });
    return this.recoveryPromise;
  }

  private async resume() {
    if (this.snapshot.state !== "online") {
      await this.recover();
      return;
    }
    try {
      if (!await this.verifyAuthentication()) return;
    } catch {
      this.goOffline();
      return;
    }
    try {
      await this.checkForApplicationUpdate();
    } catch {
      // The active release remains usable; retry its background update check later.
    }
  }

  async recover() {
    if (!this.snapshot.installed || !this.client || this.recoveryPromise) return this.recoveryPromise;
    this.recoveryPromise = this.verifyAndPrepare().finally(() => {
      this.recoveryPromise = null;
    });
    return this.recoveryPromise;
  }

  private async verifyAndPrepare() {
    this.update({ state: "checking", canMutate: false });
    try {
      if (!await this.verifyAuthentication()) return;
      await this.prepareRecovery();
    } catch {
      this.goOffline();
    }
  }

  private async verifyAuthentication() {
    const response = await fetch("/auth/status", {
      cache: "no-store",
      credentials: "same-origin"
    });
    if (response.status === 401 || response.status === 403) {
      this.requireAuthentication();
      return false;
    }
    if (!response.ok) throw new Error("Authentication status unavailable.");
    const status = (await response.json()) as { state?: string };
    if (status.state !== "authenticated") {
      this.requireAuthentication();
      return false;
    }
    await markAuthenticated();
    return true;
  }

  private async prepareRecovery() {
    this.update({ state: "checking", canMutate: false });
    try {
      await this.checkForApplicationUpdate();
      this.generation += 1;
      this.baselineCache = (this.client?.extract() as NormalizedCacheObject | undefined) ?? null;
      this.update({ state: "reconciling", canMutate: false });
    } catch {
      this.goOffline();
    }
  }

  async finishReconciliation() {
    const lastSync = Date.now();
    await this.persistNow(lastSync);
    this.baselineCache = null;
    this.update({ state: "online", lastSync, canMutate: true });
  }

  failReconciliation() {
    if (this.client && this.baselineCache) this.client.cache.restore(this.baselineCache);
    this.baselineCache = null;
    this.goOffline();
  }

  requireAuthentication() {
    this.update({ state: "auth_required", canMutate: false });
  }

  goOffline() {
    if (!this.snapshot.installed) return;
    this.update({ state: "offline", updating: false, canMutate: false });
  }

  setCriticalOperation(key: string, active: boolean) {
    if (active) this.criticalOperations.add(key);
    else this.criticalOperations.delete(key);
    if (this.criticalOperations.size === 0) {
      this.safePointWaiters.forEach((resolve) => resolve());
      this.safePointWaiters.clear();
    }
  }

  registerFlusher(flusher: () => Promise<void>) {
    this.flushers.add(flusher);
    return () => {
      this.flushers.delete(flusher);
    };
  }

  trackQuery(operationName: string, variables: Record<string, unknown>) {
    if (!this.snapshot.installed || !mutableDocuments.has(operationName)) return;
    const serialized = JSON.stringify(variables);
    this.recentQueries = this.recentQueries.filter(
      (query) => query.operationName !== operationName || JSON.stringify(query.variables) !== serialized
    );
    this.recentQueries.unshift({ operationName, variables, touchedAt: Date.now() });
    this.recentQueries = this.recentQueries.slice(0, 100);
  }

  async revalidateRecentQueries() {
    const client = this.client;
    if (!client) return;
    const queue = [...this.recentQueries];
    const worker = async () => {
      for (;;) {
        const descriptor = queue.shift();
        if (!descriptor) return;
        const query = mutableDocuments.get(descriptor.operationName);
        if (!query) continue;
        try {
          await client.query({
            query,
            variables: descriptor.variables,
            fetchPolicy: "network-only",
            errorPolicy: "all"
          });
        } catch {
          // Keep the previous object; unrelated cached reads may still reconcile.
        }
      }
    };
    await Promise.all(Array.from({ length: Math.min(4, queue.length) }, worker));
  }

  schedulePersistence() {
    if (!this.snapshot.installed || this.snapshot.state !== "online") return;
    if (this.persistenceTimer !== null) window.clearTimeout(this.persistenceTimer);
    this.persistenceTimer = window.setTimeout(() => {
      this.persistenceTimer = null;
      void this.persistNow();
    }, 400);
  }

  async persistNow(lastSync = this.snapshot.lastSync) {
    if (!this.snapshot.installed || !this.client || this.erasing) return;
    await saveDurableSnapshot({
      generation: this.generation,
      lastSync,
      cache: this.client.extract() as NormalizedCacheObject,
      recentQueries: this.recentQueries
    });
  }

  async erasePrivateData() {
    if (!this.snapshot.installed) return;
    this.erasing = true;
    if (this.persistenceTimer !== null) {
      window.clearTimeout(this.persistenceTimer);
      this.persistenceTimer = null;
    }
    await erasePwaPrivateStorage();
    this.generation += 1;
    this.recentQueries = [];
    this.baselineCache = null;
    this.client?.cache.restore({});
    this.update({ state: "auth_required", lastSync: null, updating: false, canMutate: false });
  }

  private checkForApplicationUpdate() {
    if (this.applicationUpdatePromise) return this.applicationUpdatePromise;
    this.applicationUpdatePromise = this.installApplicationUpdate().finally(() => {
      this.applicationUpdatePromise = null;
    });
    return this.applicationUpdatePromise;
  }

  private async installApplicationUpdate() {
    if (!("serviceWorker" in navigator)) return;
    const registration = await navigator.serviceWorker.register("/assets/sw.js", {
      scope: "/",
      updateViaCache: "none"
    });
    await registration.update();
    await waitForWorkerInstall(registration);
    if (!registration.waiting || !navigator.serviceWorker.controller) return;

    this.update({ updating: true, canMutate: false });
    await this.waitForSafePoint();
    await Promise.all([...this.flushers].map((flush) => flush()));
    await this.persistNow();
    const controllerChanged = waitForControllerChange();
    registration.waiting.postMessage({ type: "SKIP_WAITING" });
    await controllerChanged;
    window.location.reload();
    await new Promise<never>(() => undefined);
  }

  private waitForSafePoint() {
    if (this.criticalOperations.size === 0) return Promise.resolve();
    return new Promise<void>((resolve) => this.safePointWaiters.add(resolve));
  }

  private update(patch: Partial<PwaRuntimeSnapshot>) {
    this.snapshot = { ...this.snapshot, ...patch };
    this.listeners.forEach((listener) => listener());
  }
}

function waitForControllerChange(timeoutMs = 10_000) {
  return new Promise<void>((resolve, reject) => {
    const changed = () => {
      window.clearTimeout(timeout);
      resolve();
    };
    const timeout = window.setTimeout(() => {
      navigator.serviceWorker.removeEventListener("controllerchange", changed);
      reject(new Error("Noema update activation timed out."));
    }, timeoutMs);
    navigator.serviceWorker.addEventListener("controllerchange", changed, { once: true });
  });
}

async function waitForWorkerInstall(registration: ServiceWorkerRegistration) {
  const worker = registration.installing;
  if (!worker) return;
  if (worker.state === "installed" || worker.state === "activated") return;
  await new Promise<void>((resolve, reject) => {
    worker.addEventListener("statechange", () => {
      if (worker.state === "installed" || worker.state === "activated") resolve();
      if (worker.state === "redundant") reject(new Error("Noema update installation failed."));
    });
  });
}

export const pwaRuntime = new PwaRuntime();

export function createPwaApolloLink() {
  return new ApolloLink((operation, forward) => {
    const mutation = operation.operationType === "mutation";
    if (mutation && !pwaRuntime.getSnapshot().canMutate) {
      return new Observable((observer) => {
        observer.error(new Error("Reconnect and finish updating Noema before making changes."));
      });
    }

    if (operation.operationType === "query") {
      pwaRuntime.trackQuery(operation.operationName ?? "", operation.variables);
    }
    const generation = pwaRuntime.requestGeneration();
    const criticalKey = mutation ? `mutation:${operation.operationName}:${crypto.randomUUID()}` : null;
    if (criticalKey) pwaRuntime.setCriticalOperation(criticalKey, true);

    return new Observable((observer) => {
      const subscription = forward(operation).subscribe({
        next: (value) => {
          if (operation.operationType !== "query" || generation === pwaRuntime.requestGeneration()) {
            observer.next(value);
          } else {
            // Apollo requires every completed query to emit; retain the current cache
            // instead of allowing the stale network payload across generations.
            observer.next({
              data: operation.client.readQuery({
                query: operation.query,
                variables: operation.variables,
                returnPartialData: true
              }) ?? undefined
            });
          }
          if (operation.operationType === "query") pwaRuntime.schedulePersistence();
        },
        error: (error) => {
          if (criticalKey) pwaRuntime.setCriticalOperation(criticalKey, false);
          observer.error(error);
        },
        complete: () => {
          if (criticalKey) pwaRuntime.setCriticalOperation(criticalKey, false);
          observer.complete();
        }
      });
      return () => {
        if (criticalKey) pwaRuntime.setCriticalOperation(criticalKey, false);
        subscription.unsubscribe();
      };
    });
  });
}
