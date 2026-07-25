import React from "react";
import { ApolloProvider } from "@apollo/client/react";
import { RouterProvider } from "@tanstack/react-router";
import { createRoot } from "react-dom/client";
import { apolloClient } from "./graphql/client";
import { MotionRoot } from "./motion/MotionRoot";
import { router } from "./router";
import { AuthGate } from "./auth/AuthGate";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <MotionRoot>
      <AuthGate>
        <ApolloProvider client={apolloClient}>
          <RouterProvider router={router} />
        </ApolloProvider>
      </AuthGate>
    </MotionRoot>
  </React.StrictMode>
);
