import React from "react";
import { ApolloProvider } from "@apollo/client/react";
import { RouterProvider } from "@tanstack/react-router";
import { createRoot } from "react-dom/client";
import { createApolloClient } from "./graphql/client";
import { MotionRoot } from "./motion/MotionRoot";
import { router } from "./router";
import { AuthGate } from "./auth/AuthGate";
import "./styles.css";

const apolloClient = await createApolloClient();

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <MotionRoot>
      <ApolloProvider client={apolloClient}>
        <AuthGate>
          <RouterProvider router={router} />
        </AuthGate>
      </ApolloProvider>
    </MotionRoot>
  </React.StrictMode>
);
