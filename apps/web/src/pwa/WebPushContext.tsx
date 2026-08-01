import * as React from "react";
import { useMutation, useQuery, useSubscription } from "@apollo/client/react";
import {
  RegisterWebPushSubscriptionDocument,
  RemoveWebPushSubscriptionDocument,
  WebPushPresenceDocument,
  WebPushStatusDocument
} from "@/generated/graphql";

const DISMISSED_PROMPT_KEY = "noema.web-push-prompt-dismissed";

type WebPushState = "unavailable" | "loading" | "disabled" | "enabled" | "denied";

type WebPushContextValue = {
  state: WebPushState;
  detail: string;
  error: string | null;
  promptEligible: boolean;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
  dismissPrompt: () => void;
};

const WebPushContext = React.createContext<WebPushContextValue | null>(null);

export function WebPushProvider({
  installed,
  chatVisible,
  connectionReady,
  children
}: {
  installed: boolean;
  chatVisible: boolean;
  connectionReady: boolean;
  children: React.ReactNode;
}) {
  const capable = installed && supportsWebPush();
  const [subscription, setSubscription] = React.useState<PushSubscription | null>(null);
  const [initialized, setInitialized] = React.useState(!capable);
  const [working, setWorking] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [promptDismissed, setPromptDismissed] = React.useState(readPromptDismissed);
  const [focused, setFocused] = React.useState(readFocused);
  const endpoint = subscription?.endpoint;
  const statusResult = useQuery(WebPushStatusDocument, {
    variables: { endpoint },
    skip: !capable,
    fetchPolicy: "network-only"
  });
  const [registerSubscription] = useMutation(RegisterWebPushSubscriptionDocument);
  const [removeSubscription] = useMutation(RemoveWebPushSubscriptionDocument);
  const repairAttemptedEndpoint = React.useRef<string | null>(null);

  React.useEffect(() => {
    if (!capable) {
      return;
    }
    let active = true;
    void navigator.serviceWorker.getRegistration("/").then(async (registration) => {
      const existing = await registration?.pushManager.getSubscription() ?? null;
      if (active) {
        setSubscription(existing);
        setInitialized(true);
      }
    }).catch(() => {
      if (active) setInitialized(true);
    });
    return () => {
      active = false;
    };
  }, [capable]);

  React.useEffect(() => {
    const update = () => setFocused(readFocused());
    window.addEventListener("focus", update);
    window.addEventListener("blur", update);
    window.addEventListener("pageshow", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      window.removeEventListener("focus", update);
      window.removeEventListener("blur", update);
      window.removeEventListener("pageshow", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, []);

  const serverStatus = statusResult.data?.webPushStatus;
  const subscriptionId = serverStatus?.subscriptionId ?? null;

  React.useEffect(() => {
    if (
      !subscription ||
      !initialized ||
      statusResult.loading ||
      !serverStatus?.available ||
      subscriptionId ||
      repairAttemptedEndpoint.current === subscription.endpoint
    ) return;
    repairAttemptedEndpoint.current = subscription.endpoint;
    void registerSubscription({ variables: { input: browserSubscriptionInput(subscription) } })
      .then(() => statusResult.refetch({ endpoint: subscription.endpoint }))
      .catch(() => setError("Noema could not restore notifications for this device."));
  }, [initialized, registerSubscription, serverStatus, statusResult, subscription, subscriptionId]);

  useSubscription(WebPushPresenceDocument, {
    variables: { subscriptionId: subscriptionId ?? "" },
    skip:
      !subscriptionId ||
      !chatVisible ||
      !connectionReady ||
      !focused ||
      Notification.permission !== "granted"
  });

  const enable = React.useCallback(async () => {
    setError(null);
    if (!capable) return;
    const applicationServerKey = serverStatus?.applicationServerKey;
    if (!applicationServerKey) {
      setError(serverStatus?.blocker ?? "Notifications are not available yet.");
      return;
    }
    setWorking(true);
    try {
      const permission = await Notification.requestPermission();
      if (permission !== "granted") return;
      const registration = await navigator.serviceWorker.register("/assets/sw.js", {
        scope: "/",
        updateViaCache: "none"
      });
      await navigator.serviceWorker.ready;
      const next = await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: decodeApplicationServerKey(applicationServerKey)
      });
      await registerSubscription({ variables: { input: browserSubscriptionInput(next) } });
      setSubscription(next);
      setPromptDismissed(false);
      window.localStorage.removeItem(DISMISSED_PROMPT_KEY);
      await statusResult.refetch({ endpoint: next.endpoint });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Noema could not enable notifications.");
    } finally {
      setWorking(false);
    }
  }, [capable, registerSubscription, serverStatus, statusResult]);

  const disable = React.useCallback(async () => {
    if (!subscription || !subscriptionId) return;
    setError(null);
    setWorking(true);
    try {
      await removeSubscription({ variables: { subscriptionId } });
      await subscription.unsubscribe();
      setSubscription(null);
      await statusResult.refetch({ endpoint: undefined });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Noema could not disable notifications.");
    } finally {
      setWorking(false);
    }
  }, [removeSubscription, statusResult, subscription, subscriptionId]);

  const permission = supportsNotifications() ? Notification.permission : "default";
  const state: WebPushState = !capable || serverStatus?.available === false
    ? "unavailable"
    : permission === "denied"
      ? "denied"
      : working || !initialized || statusResult.loading || !serverStatus
        ? "loading"
        : subscriptionId
          ? "enabled"
          : "disabled";
  const detail = stateDetail(state, installed, serverStatus?.blocker ?? null);
  const dismissPrompt = React.useCallback(() => {
    window.localStorage.setItem(DISMISSED_PROMPT_KEY, "true");
    setPromptDismissed(true);
  }, []);
  const value = React.useMemo<WebPushContextValue>(() => ({
    state,
    detail,
    error,
    promptEligible: state === "disabled" && permission === "default" && !promptDismissed,
    enable,
    disable,
    dismissPrompt
  }), [detail, disable, dismissPrompt, enable, error, permission, promptDismissed, state]);

  return <WebPushContext.Provider value={value}>{children}</WebPushContext.Provider>;
}

export function useWebPush() {
  const value = React.useContext(WebPushContext);
  if (!value) throw new Error("useWebPush must be used within WebPushProvider");
  return value;
}

function supportsNotifications() {
  return typeof window !== "undefined" && "Notification" in window;
}

function supportsWebPush() {
  return supportsNotifications() && "serviceWorker" in navigator && "PushManager" in window;
}

function readFocused() {
  return typeof document !== "undefined" && document.visibilityState === "visible" && document.hasFocus();
}

function readPromptDismissed() {
  return typeof window !== "undefined" && window.localStorage.getItem(DISMISSED_PROMPT_KEY) === "true";
}

function stateDetail(state: WebPushState, installed: boolean, blocker: string | null) {
  switch (state) {
    case "enabled":
      return "Noema will notify this device for primary chat replies and items that need you.";
    case "denied":
      return "Notifications are blocked in this device's system settings.";
    case "disabled":
      return "Enable alerts for primary chat replies and items that need you.";
    case "loading":
      return "Checking this device's notification settings…";
    case "unavailable":
      return installed
        ? blocker ?? "This browser does not support Web Push notifications."
        : "Install Noema to your Home Screen or as an app before enabling notifications.";
  }
}

function browserSubscriptionInput(subscription: PushSubscription) {
  const json = subscription.toJSON();
  if (!json.endpoint || !json.keys?.p256dh || !json.keys.auth) {
    throw new Error("This browser returned an incomplete notification subscription.");
  }
  return {
    endpoint: json.endpoint,
    p256Dh: json.keys.p256dh,
    auth: json.keys.auth
  };
}

function decodeApplicationServerKey(value: string) {
  const padding = "=".repeat((4 - value.length % 4) % 4);
  const binary = window.atob((value + padding).replaceAll("-", "+").replaceAll("_", "/"));
  return Uint8Array.from(binary, (character) => character.charCodeAt(0));
}
