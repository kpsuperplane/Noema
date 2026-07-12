import React from "react";
import { browserGraphqlConnectionMonitor } from "@/graphql/browserTransport";

export function useBrowserGraphqlRecovery({
  enabled,
  onConnecting,
  onReady,
  onRecovered
}: {
  enabled: boolean;
  onConnecting: () => void;
  onReady: () => void;
  onRecovered: () => void;
}) {
  const connection = React.useSyncExternalStore(
    browserGraphqlConnectionMonitor.subscribe,
    browserGraphqlConnectionMonitor.getSnapshot,
    browserGraphqlConnectionMonitor.getSnapshot
  );
  const handledRecoverySequenceRef = React.useRef(connection.recoverySequence);

  React.useEffect(() => {
    if (!enabled) {
      return;
    }

    let cancelled = false;
    window.queueMicrotask(() => {
      if (cancelled) {
        return;
      }
      if (connection.state !== "ready") {
        onConnecting();
        return;
      }

      onReady();
      if (connection.recoverySequence !== handledRecoverySequenceRef.current) {
        handledRecoverySequenceRef.current = connection.recoverySequence;
        onRecovered();
      }
    });
    return () => {
      cancelled = true;
    };
  }, [connection.recoverySequence, connection.state, enabled, onConnecting, onReady, onRecovered]);
}
