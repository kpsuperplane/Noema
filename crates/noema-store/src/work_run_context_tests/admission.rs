use super::*;

#[tokio::test]
async fn context_admission_consumes_exact_messages_and_replays_the_checkpoint() {
    let fixture = fixture().await;
    fixture
        .store
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO task_gates (gate_id, task_id, task_generation, contract_id, gate_kind, gate_state, prompt_markdown, context_markdown, opened_by_actor_id, originating_run_id) VALUES ('gate:admission', ?1, ?2, ?3, 'clarification', 'open', 'Question', 'Context', ?4, ?5)",
                rusqlite::params![fixture.task.task_id.as_str(), fixture.task.generation, fixture.contract.contract_id.as_str(), ACTOR, fixture.planner_run_id],
            ).map_err(StoreError::Sqlite)?;
            connection.execute(
                "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, gate_id, message_kind, body_markdown, author_actor_id) VALUES ('task_message:admitted', ?1, ?2, ?3, 'gate:admission', 'human_answer', 'Exact answer', ?4)",
                rusqlite::params![fixture.task.task_id.as_str(), fixture.task.generation, fixture.contract.contract_id.as_str(), ACTOR],
            ).map_err(StoreError::Sqlite)?;
            Ok(())
        })
        .await
        .expect("insert admission evidence");

    let first = fixture
        .service
        .admit_work_run_execution_context(
            &fixture.executor_fence,
            ACTOR,
            None,
            "correlation:context:admission",
        )
        .await
        .expect("admit context");
    assert_eq!(first.checkpoint.kind, AgentRunItemKind::ContextCheckpoint);
    assert_eq!(first.context.messages.len(), 1);
    assert_eq!(
        first.context.messages[0].consumed_by_run_id.as_deref(),
        Some(fixture.executor_fence.run_id.as_str())
    );

    fixture
        .store
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, gate_id, message_kind, body_markdown, author_actor_id) VALUES ('task_message:late', ?1, ?2, ?3, 'gate:admission', 'human_answer', 'Late answer', ?4)",
                rusqlite::params![fixture.task.task_id.as_str(), fixture.task.generation, fixture.contract.contract_id.as_str(), ACTOR],
            ).map_err(StoreError::Sqlite)
        })
        .await
        .expect("insert post-checkpoint message");
    let replay = fixture
        .service
        .admit_work_run_execution_context(
            &fixture.executor_fence,
            ACTOR,
            None,
            "correlation:context:response-loss-retry",
        )
        .await
        .expect("replay admission");
    assert_eq!(replay.checkpoint.item_id, first.checkpoint.item_id);
    assert_eq!(replay.context.messages.len(), 1);
    assert_eq!(
        replay.context.messages[0].message_id.as_str(),
        "task_message:admitted"
    );
    let late_consumer: Option<String> = fixture
        .store
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT consumed_by_run_id FROM task_messages WHERE message_id = 'task_message:late'",
                    [],
                    |row| row.get(0),
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("load late consumer");
    assert_eq!(late_consumer, None);
}

#[tokio::test]
async fn checkpoint_replay_rejects_cross_task_identity() {
    let fixture = fixture().await;
    let admitted = fixture
        .service
        .admit_work_run_execution_context(
            &fixture.executor_fence,
            ACTOR,
            None,
            "correlation:context:identity-admission",
        )
        .await
        .expect("admit context");
    fixture
        .store
        .with_connection(|connection| {
            let payload_json: String = connection.query_row(
                "SELECT payload_json FROM agent_run_items WHERE item_id = ?1",
                [admitted.checkpoint.item_id.as_str()],
                |row| row.get(0),
            )?;
            let mut payload: serde_json::Value = serde_json::from_str(&payload_json)?;
            payload["context"]["task"]["task_id"] = serde_json::json!("task:forged");
            connection.execute(
                "UPDATE agent_run_items SET payload_json = ?2 WHERE item_id = ?1",
                rusqlite::params![admitted.checkpoint.item_id, payload.to_string()],
            )?;
            Ok(())
        })
        .await
        .expect("forge checkpoint task identity");
    let error = fixture
        .service
        .admit_work_run_execution_context(
            &fixture.executor_fence,
            ACTOR,
            None,
            "correlation:context:identity-replay",
        )
        .await
        .expect_err("cross-task checkpoint must fail closed");
    assert_invariant(error);
}

#[tokio::test]
async fn recovery_children_inherit_the_admitted_answer_across_multiple_lease_expiries() {
    let fixture = fixture().await;
    fixture
        .store
        .with_connection(|connection| {
            connection.execute(
                "INSERT INTO task_gates (gate_id, task_id, task_generation, contract_id, gate_kind, gate_state, prompt_markdown, context_markdown, opened_by_actor_id, originating_run_id) VALUES ('gate:recovery-lineage', ?1, ?2, ?3, 'clarification', 'open', 'Question', 'Context', ?4, ?5)",
                rusqlite::params![fixture.task.task_id.as_str(), fixture.task.generation, fixture.contract.contract_id.as_str(), ACTOR, fixture.executor_fence.run_id],
            ).map_err(StoreError::Sqlite)?;
            connection.execute(
                "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, gate_id, message_kind, body_markdown, author_actor_id) VALUES ('task_message:recovery-answer', ?1, ?2, ?3, 'gate:recovery-lineage', 'human_answer', 'Retain this exact answer', ?4)",
                rusqlite::params![fixture.task.task_id.as_str(), fixture.task.generation, fixture.contract.contract_id.as_str(), ACTOR],
            ).map_err(StoreError::Sqlite)?;
            Ok(())
        })
        .await
        .expect("seed recovery answer");
    let parent = fixture
        .service
        .admit_work_run_execution_context(
            &fixture.executor_fence,
            ACTOR,
            None,
            "correlation:context:recovery-parent",
        )
        .await
        .expect("admit parent context");
    assert_eq!(parent.context.messages.len(), 1);

    fixture
        .store
        .with_connection(|connection| {
            connection.execute(
                "UPDATE workspaces SET name = 'Mutable workspace' WHERE workspace_id = ?1",
                [WORKSPACE],
            ).map_err(StoreError::Sqlite)?;
            connection.execute(
                "INSERT INTO task_messages (message_id, task_id, task_generation, contract_id, gate_id, message_kind, body_markdown, author_actor_id) VALUES ('task_message:late-recovery-evidence', ?1, ?2, ?3, 'gate:recovery-lineage', 'human_answer', 'Late mutable evidence', ?4)",
                rusqlite::params![fixture.task.task_id.as_str(), fixture.task.generation, fixture.contract.contract_id.as_str(), ACTOR],
            ).map_err(StoreError::Sqlite)?;
            connection.execute(
                "UPDATE agent_runs SET lease_expires_at = '2000-01-01T00:00:00.000Z' WHERE run_id = ?1",
                [fixture.executor_fence.run_id.as_str()],
            ).map_err(StoreError::Sqlite)?;
            Ok(())
        })
        .await
        .expect("expire parent after mutable edits");

    let child_claim = fixture
        .service
        .claim_next_work_run("worker:context:recovery-child", 60, &[])
        .await
        .expect("recover and claim child")
        .expect("recovery child");
    let child_fence = WorkRunFence {
        run_id: child_claim.run.run_id.clone(),
        lease_token: child_claim.lease_token,
        task_generation: fixture.task.generation,
        contract_id: Some(fixture.contract.contract_id.clone()),
    };
    fixture
        .service
        .start_work_run(
            &child_fence,
            ACTOR,
            None,
            "correlation:context:recovery-child",
        )
        .await
        .expect("start child");
    let child = fixture
        .service
        .admit_work_run_execution_context(
            &child_fence,
            ACTOR,
            None,
            "correlation:context:recovery-child-admit",
        )
        .await
        .expect("admit child context");
    assert_eq!(child.context.workspace, parent.context.workspace);
    assert_eq!(child.context.project, parent.context.project);
    assert_eq!(child.context.messages, parent.context.messages);

    fixture
        .store
        .with_connection(|connection| {
            connection
                .execute(
                    "UPDATE agent_runs SET lease_expires_at = '2000-01-01T00:00:00.000Z' WHERE run_id = ?1",
                    [child_fence.run_id.as_str()],
                )
                .map_err(StoreError::Sqlite)
        })
        .await
        .expect("expire child");
    let grandchild_claim = fixture
        .service
        .claim_next_work_run("worker:context:recovery-grandchild", 60, &[])
        .await
        .expect("recover and claim grandchild")
        .expect("recovery grandchild");
    let grandchild_fence = WorkRunFence {
        run_id: grandchild_claim.run.run_id,
        lease_token: grandchild_claim.lease_token,
        task_generation: fixture.task.generation,
        contract_id: Some(fixture.contract.contract_id.clone()),
    };
    fixture
        .service
        .start_work_run(
            &grandchild_fence,
            ACTOR,
            None,
            "correlation:context:recovery-grandchild",
        )
        .await
        .expect("start grandchild");
    let grandchild = fixture
        .service
        .admit_work_run_execution_context(
            &grandchild_fence,
            ACTOR,
            None,
            "correlation:context:recovery-grandchild-admit",
        )
        .await
        .expect("admit grandchild context");
    assert_eq!(grandchild.context.workspace, parent.context.workspace);
    assert_eq!(grandchild.context.project, parent.context.project);
    assert_eq!(grandchild.context.messages, parent.context.messages);
}
