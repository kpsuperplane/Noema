import { createGraphiQLFetcher } from "@graphiql/toolkit";
import { GraphiQL } from "graphiql";
import "graphiql/style.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./graphiql.css";

const socketProtocol = window.location.protocol === "https:" ? "wss:" : "ws:";
const fetcher = createGraphiQLFetcher({
  url: "/graphql",
  subscriptionUrl: `${socketProtocol}//${window.location.host}/graphql/ws`
});
const root = document.getElementById("root");

if (!root) {
  throw new Error("GraphiQL root is missing");
}

createRoot(root).render(
  <StrictMode>
    <GraphiQL fetcher={fetcher} />
  </StrictMode>
);
