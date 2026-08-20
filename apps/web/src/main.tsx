import React from "react";
import { ApolloProvider } from "@apollo/client/react";
import { RouterProvider } from "@tanstack/react-router";
import { createRoot } from "react-dom/client";
import { createApolloClient } from "./graphql/client";
import { MotionRoot } from "./motion/MotionRoot";
import { router } from "./router";
import { AuthGate } from "./auth/AuthGate";
import { DesktopConnectionGate } from "./desktop/DesktopConnectionGate";
import {
  AppBootstrapError,
  AppFatalBoundary
} from "./components/shell/AppBootBoundary";
import {
  reportCaughtReactError,
  reportReactError
} from "./components/errors/RenderErrorBoundary";
import "./styles.css";

const rootElement = document.getElementById("root");
if (!rootElement) {
  throw new Error("Noema root is missing.");
}

const root = createRoot(rootElement, {
  onCaughtError: reportCaughtReactError,
  onUncaughtError: (error, info) =>
    reportReactError("react.uncaught", error, info.componentStack),
  onRecoverableError: (error, info) =>
    reportReactError("react.recoverable", error, info.componentStack)
});

try {
  const apolloClient = await createApolloClient();
  root.render(
    <React.StrictMode>
      <AppFatalBoundary>
        <MotionRoot>
          <DesktopConnectionGate>
            <ApolloProvider client={apolloClient}>
              <AuthGate>
                <RouterProvider router={router} />
              </AuthGate>
            </ApolloProvider>
          </DesktopConnectionGate>
        </MotionRoot>
      </AppFatalBoundary>
    </React.StrictMode>
  );
} catch (error) {
  reportReactError("app.bootstrap", error);
  root.render(<AppBootstrapError />);
}
