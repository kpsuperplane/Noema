import { gql } from "@apollo/client";

export const ClientsDocument = gql`
  query Clients {
    clients {
      clientId
      displayName
      createdAt
      revokedAt
      isCurrent
    }
  }
`;

export const RevokeClientDocument = gql`
  mutation RevokeClient($clientId: String!) {
    revokeClient(clientId: $clientId) { clientId }
  }
`;

export const RevokeAllClientsDocument = gql`
  mutation RevokeAllClients {
    revokeAllClients
  }
`;
