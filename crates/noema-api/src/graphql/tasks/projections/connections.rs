use async_graphql::SimpleObject;

use super::*;

/// Task edge.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskEdge")]
pub struct GraphqlTaskEdge {
    /// Opaque keyset cursor.
    pub cursor: String,
    /// Task node.
    pub node: GraphqlTaskSummary,
}

/// Task connection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskConnection")]
pub struct GraphqlTaskConnection {
    /// Ordered task edges.
    pub edges: Vec<GraphqlTaskEdge>,
    /// Pagination metadata.
    pub page_info: GraphqlPageInfo,
}

/// Project edge.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProjectEdge")]
pub struct GraphqlProjectEdge {
    /// Opaque keyset cursor.
    pub cursor: String,
    /// Project node.
    pub node: GraphqlProject,
}

/// Project connection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProjectConnection")]
pub struct GraphqlProjectConnection {
    /// Ordered project edges.
    pub edges: Vec<GraphqlProjectEdge>,
    /// Pagination metadata.
    pub page_info: GraphqlPageInfo,
}

/// Event edge.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WorkEventEdge")]
pub struct GraphqlWorkEventEdge {
    /// Opaque event cursor.
    pub cursor: String,
    /// Event node.
    pub node: GraphqlWorkEvent,
}

/// Work event connection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "WorkEventConnection")]
pub struct GraphqlWorkEventConnection {
    /// Ordered event edges.
    pub edges: Vec<GraphqlWorkEventEdge>,
    /// Pagination metadata.
    pub page_info: GraphqlPageInfo,
}

/// Derived attention edge.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskAttentionEdge")]
pub struct GraphqlTaskAttentionEdge {
    /// Opaque attention cursor.
    pub cursor: String,
    /// Attention node.
    pub node: GraphqlTaskAttention,
}

/// Derived attention connection.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskAttentionConnection")]
pub struct GraphqlTaskAttentionConnection {
    /// Ordered attention edges.
    pub edges: Vec<GraphqlTaskAttentionEdge>,
    /// Pagination metadata.
    pub page_info: GraphqlPageInfo,
}

macro_rules! history_connection {
    ($edge:ident, $connection:ident, $node:ty, $edge_name:literal, $connection_name:literal) => {
        #[derive(Clone, Debug, SimpleObject)]
        #[graphql(name = $edge_name)]
        pub struct $edge {
            /// Opaque history cursor.
            pub cursor: String,
            /// History node.
            pub node: $node,
        }

        #[derive(Clone, Debug, SimpleObject)]
        #[graphql(name = $connection_name)]
        pub struct $connection {
            /// Ordered history edges.
            pub edges: Vec<$edge>,
            /// Pagination metadata.
            pub page_info: GraphqlPageInfo,
        }
    };
}

history_connection!(
    GraphqlTaskExecutionContractEdge,
    GraphqlTaskExecutionContractConnection,
    GraphqlTaskExecutionContract,
    "TaskExecutionContractEdge",
    "TaskExecutionContractConnection"
);
history_connection!(
    GraphqlTaskGateEdge,
    GraphqlTaskGateConnection,
    GraphqlTaskGate,
    "TaskGateEdge",
    "TaskGateConnection"
);
history_connection!(
    GraphqlTaskMessageEdge,
    GraphqlTaskMessageConnection,
    GraphqlTaskMessage,
    "TaskMessageEdge",
    "TaskMessageConnection"
);
history_connection!(
    GraphqlTaskRunEdge,
    GraphqlTaskRunConnection,
    GraphqlTaskRun,
    "TaskRunEdge",
    "TaskRunConnection"
);
history_connection!(
    GraphqlTaskSubmissionEdge,
    GraphqlTaskSubmissionConnection,
    GraphqlTaskSubmission,
    "TaskSubmissionEdge",
    "TaskSubmissionConnection"
);
history_connection!(
    GraphqlTaskReviewEdge,
    GraphqlTaskReviewConnection,
    GraphqlTaskReview,
    "TaskReviewEdge",
    "TaskReviewConnection"
);
history_connection!(
    GraphqlTaskArtifactEdge,
    GraphqlTaskArtifactConnection,
    crate::graphql::artifacts::GraphqlArtifact,
    "TaskArtifactEdge",
    "TaskArtifactConnection"
);
history_connection!(
    GraphqlTaskRunItemEdge,
    GraphqlTaskRunItemConnection,
    GraphqlTaskRunItem,
    "TaskRunItemEdge",
    "TaskRunItemConnection"
);

/// Project mutation payload.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "ProjectCommandPayload")]
pub struct GraphqlProjectCommandPayload {
    /// Authoritative project projection.
    pub project: GraphqlProject,
    /// Cursor for the event committed by the command.
    pub event_cursor: String,
    /// Echoed caller idempotency key.
    pub client_mutation_id: String,
}

/// Task mutation payload.
#[derive(Clone, Debug, SimpleObject)]
#[graphql(name = "TaskCommandPayload")]
pub struct GraphqlTaskCommandPayload {
    /// Authoritative task projection.
    pub task: GraphqlTaskDetail,
    /// Cursor for the event committed by the command.
    pub event_cursor: String,
    /// Echoed caller idempotency key.
    pub client_mutation_id: String,
}
