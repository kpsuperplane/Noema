import type { NormalizedCacheObject } from "@apollo/client";
import { isTauriRuntime } from "@/graphql/transportMode";

const DATABASE_NAME = "noema-pwa";
const DATABASE_VERSION = 1;
const STORE_NAME = "records";
const CACHE_SCHEMA_VERSION = 3;

type StoredRecord<T> = { key: string; value: T };

export type CachedQuery = {
  operationName: string;
  variables: Record<string, unknown>;
  touchedAt: number;
};

export type DurableSnapshot = {
  cacheSchemaVersion: number;
  generation: number;
  lastSync: number | null;
  cache: NormalizedCacheObject;
  recentQueries: CachedQuery[];
};

export type TaskCaptureDraft = {
  title: string;
  taskDocument: string;
  projectId: string;
};

export function isInstalledPwa() {
  if (isTauriRuntime() || typeof window === "undefined") return false;
  const navigatorWithStandalone = navigator as Navigator & { standalone?: boolean };
  return window.matchMedia("(display-mode: standalone)").matches || navigatorWithStandalone.standalone === true;
}

export async function loadDurableSnapshot(): Promise<DurableSnapshot | null> {
  if (!isInstalledPwa()) return null;
  const snapshot = await readRecord<DurableSnapshot>("snapshot");
  return snapshot?.cacheSchemaVersion === CACHE_SCHEMA_VERSION ? snapshot : null;
}

export async function saveDurableSnapshot(snapshot: Omit<DurableSnapshot, "cacheSchemaVersion">) {
  if (!isInstalledPwa()) return;
  await writeRecords([
    {
      key: "snapshot",
      value: { ...snapshot, cacheSchemaVersion: CACHE_SCHEMA_VERSION }
    }
  ]);
}

export async function hasAuthenticatedSentinel() {
  return isInstalledPwa() && (await readRecord<boolean>("authenticated")) === true;
}

export async function markAuthenticated() {
  if (!isInstalledPwa()) return;
  await writeRecords([{ key: "authenticated", value: true }]);
}

export async function readChatDraft(conversationId: string) {
  return (await readRecord<string>(`chat-draft:${conversationId}`)) ?? "";
}

export async function writeChatDraft(conversationId: string, value: string) {
  if (!isInstalledPwa()) return;
  await writeRecords([{ key: `chat-draft:${conversationId}`, value }]);
}

export async function readTaskCaptureDraft() {
  const draft = await readRecord<TaskCaptureDraft | { title: string; description: string; projectId: string }>("task-capture");
  if (!draft) return null;
  if ("description" in draft) {
    const converted = {
      title: draft.title,
      taskDocument: draft.description,
      projectId: draft.projectId
    };
    await writeTaskCaptureDraft(converted);
    return converted;
  }
  return draft;
}

export async function writeTaskCaptureDraft(value: TaskCaptureDraft) {
  if (!isInstalledPwa()) return;
  await writeRecords([{ key: "task-capture", value }]);
}

export async function requestPersistentStorage() {
  if (!isInstalledPwa() || !navigator.storage?.persist) return false;
  try {
    return await navigator.storage.persist();
  } catch {
    return false;
  }
}

export async function erasePwaPrivateStorage() {
  if (!isInstalledPwa()) return;
  const database = await databasePromise?.catch(() => null);
  database?.close();
  databasePromise = null;
  await new Promise<void>((resolve, reject) => {
    const request = indexedDB.deleteDatabase(DATABASE_NAME);
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error ?? new Error("Noema could not erase offline storage."));
    request.onblocked = () => reject(new Error("Close other Noema windows, then try again."));
  });
  window.localStorage.clear();
}

let databasePromise: Promise<IDBDatabase> | null = null;

function openDatabase() {
  databasePromise ??= new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(DATABASE_NAME, DATABASE_VERSION);
    request.onupgradeneeded = () => {
      if (!request.result.objectStoreNames.contains(STORE_NAME)) {
        request.result.createObjectStore(STORE_NAME, { keyPath: "key" });
      }
    };
    request.onsuccess = () => {
      request.result.onversionchange = () => request.result.close();
      resolve(request.result);
    };
    request.onerror = () => reject(request.error ?? new Error("Could not open Noema offline storage."));
  });
  return databasePromise;
}

async function readRecord<T>(key: string): Promise<T | null> {
  try {
    const database = await openDatabase();
    return await new Promise<T | null>((resolve, reject) => {
      const transaction = database.transaction(STORE_NAME, "readonly");
      const request = transaction.objectStore(STORE_NAME).get(key);
      request.onsuccess = () => resolve((request.result as StoredRecord<T> | undefined)?.value ?? null);
      request.onerror = () => reject(request.error);
    });
  } catch {
    return null;
  }
}

async function writeRecords(records: StoredRecord<unknown>[]) {
  try {
    const database = await openDatabase();
    await new Promise<void>((resolve, reject) => {
      const transaction = database.transaction(STORE_NAME, "readwrite");
      const store = transaction.objectStore(STORE_NAME);
      records.forEach((record) => store.put(record));
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
  } catch {
    // Storage denial and quota exhaustion leave the server as the canonical source.
  }
}
