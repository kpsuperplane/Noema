import React from "react";
import { ApolloProvider } from "@apollo/client/react";
import { createRoot } from "react-dom/client";
import { App } from "./app/App";
import { apolloClient } from "./graphql/client";
import "./styles.css";

createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ApolloProvider client={apolloClient}>
      <App />
    </ApolloProvider>
  </React.StrictMode>
);
