use super::*;

macro_rules! connection {
    ($edge:ident, $connection:ident, $node:ty, $edge_name:literal, $connection_name:literal, $cursor_doc:literal, $nodes_doc:literal) => {
        graphql_object! { "Connection edge." => pub struct $edge($edge_name) {
            $cursor_doc => cursor: String,
            "Connection node." => node: $node,
        } }
        graphql_object! { "Bounded connection." => pub struct $connection($connection_name) {
            $nodes_doc => edges: Vec<$edge>,
            "Pagination metadata." => page_info: GraphqlPageInfo,
        } }
    };
}

connection!(
    GraphqlTaskEdge,
    GraphqlTaskConnection,
    GraphqlTaskSummary,
    "TaskEdge",
    "TaskConnection",
    "Opaque keyset cursor.",
    "Ordered task edges."
);
connection!(
    GraphqlProjectEdge,
    GraphqlProjectConnection,
    GraphqlProject,
    "ProjectEdge",
    "ProjectConnection",
    "Opaque keyset cursor.",
    "Ordered project edges."
);
connection!(
    GraphqlTaskEventEdge,
    GraphqlTaskEventConnection,
    GraphqlTaskEvent,
    "TasksEventEdge",
    "TasksEventConnection",
    "Opaque event cursor.",
    "Ordered event edges."
);
connection!(
    GraphqlTaskAttentionEdge,
    GraphqlTaskAttentionConnection,
    GraphqlTaskAttention,
    "TaskAttentionEdge",
    "TaskAttentionConnection",
    "Opaque attention cursor.",
    "Ordered attention edges."
);

connection!(
    GraphqlTaskRunItemEdge,
    GraphqlTaskRunItemConnection,
    GraphqlTaskRunItem,
    "TaskRunItemEdge",
    "TaskRunItemConnection",
    "Opaque history cursor.",
    "Ordered history edges."
);

graphql_object! { "Project mutation payload." => pub struct GraphqlProjectCommandPayload("ProjectCommandPayload") {
    "Authoritative project projection." => project: GraphqlProject,
    "Cursor for the event committed by the command." => event_cursor: String,
    "Echoed caller idempotency key." => client_mutation_id: String,
} }
graphql_object! { "Task mutation payload." => pub struct GraphqlTaskCommandPayload("TaskCommandPayload") {
    "Authoritative task projection." => task: GraphqlTaskDetail,
    "Cursor for the event committed by the command." => event_cursor: String,
    "Echoed caller idempotency key." => client_mutation_id: String,
} }
