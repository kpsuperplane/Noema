use super::*;

const ASSISTANT_RESPONSE: &str = "Got it.";

#[test]
fn valid_low_risk_preference_promotes_active() {
    let user_input = "I prefer terse Rust code review summaries.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "The current human prefers terse Rust code review summaries.",
          "memory_type": "preference",
          "title": "Rust review summary preference",
          "confidence": 0.93,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": "human:local",
              "kind": "human",
              "name": "current human",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["rust", "code review"],
            "keywords": ["terse", "summaries"],
            "summary": "Rust review response preference."
          },
          "risk_flags": [],
          "evidence_excerpt": "I prefer terse Rust code review summaries."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Active);
    assert_eq!(proposals[0].proposal.memory_type, MemoryType::Preference);
}

#[test]
fn assistant_evidence_must_be_within_one_item() {
    let response = ExtractorMemoryResponse {
        proposals: vec![
            serde_json::from_str(
                r#"{
              "content": "Noema should remember the assistant note.",
              "memory_type": "note",
              "title": "Assistant note",
              "confidence": 0.78,
              "sensitivity": "normal",
              "subjects": [
                {
                  "id": "human:local",
                  "kind": "human",
                  "name": "Kevin",
                  "role": "about"
                }
              ],
              "retrieval_hints": {
                "topics": ["memory"],
                "keywords": ["assistant note"],
                "summary": "Noema should remember the assistant note."
              },
              "risk_flags": [],
              "evidence_excerpt": "first assistant text\n\nsecond assistant text"
            }"#,
            )
            .expect("proposal"),
        ],
    };

    let error = validate_memory_extraction_response_with_assistant_items(
        response,
        "Please save the assistant note.",
        &["first assistant text", "second assistant text"],
    )
    .expect_err("split assistant evidence must reject");

    assert!(matches!(
        error,
        MemoryExtractionError::InvalidProposal {
            reason: "evidence_excerpt must exactly quote the original turn",
            ..
        }
    ));
}

#[test]
fn partition_keeps_valid_proposals_and_reports_invalid_drafts() {
    let response: ExtractorMemoryResponse = serde_json::from_str(
        r#"{
          "proposals": [
            {
              "content": "Kevin likes planes.",
              "memory_type": "preference",
              "title": "Plane preference",
              "confidence": 0.91,
              "sensitivity": "normal",
              "subjects": [
                {
                  "id": "human:local",
                  "kind": "human",
                  "name": "Kevin",
                  "role": "about"
                }
              ],
              "retrieval_hints": {
                "topics": ["aviation"],
                "keywords": ["planes"],
                "summary": "Kevin likes planes."
              },
              "risk_flags": [],
              "evidence_excerpt": "I like planes"
            },
            {
              "content": "Kevin likes helicopters.",
              "memory_type": "preference",
              "title": "Helicopter preference",
              "confidence": 0.91,
              "sensitivity": "normal",
              "subjects": [
                {
                  "id": "human:local",
                  "kind": "human",
                  "name": "Kevin",
                  "role": "about"
                }
              ],
              "retrieval_hints": {
                "topics": ["aviation"],
                "keywords": ["helicopters"],
                "summary": "Kevin likes helicopters."
              },
              "risk_flags": [],
              "evidence_excerpt": "I like helicopters"
            }
          ]
        }"#,
    )
    .expect("response");

    let partition = partition_memory_extraction_response_with_assistant_items(
        response,
        "I like planes, but please include one bad proposal fixture.",
        &["fake answer"],
    );

    assert_eq!(partition.accepted.len(), 1);
    assert_eq!(partition.accepted[0].proposal_index, 0);
    assert_eq!(partition.accepted[0].proposal.status, MemoryStatus::Active);
    assert_eq!(partition.rejected.len(), 1);
    assert_eq!(partition.rejected[0].proposal_index, 1);
    assert!(
        partition.rejected[0]
            .error
            .contains("evidence_excerpt must exactly quote the original turn")
    );
}

#[test]
fn assistant_evidence_cannot_establish_local_human_preference() {
    let response: ExtractorMemoryResponse = serde_json::from_str(
        r#"{
          "proposals": [
            {
              "content": "Kevin likes planes.",
              "memory_type": "preference",
              "title": "Plane preference",
              "confidence": 0.91,
              "sensitivity": "normal",
              "subjects": [
                {
                  "id": "human:local",
                  "kind": "human",
                  "name": "Kevin",
                  "role": "about"
                }
              ],
              "retrieval_hints": {
                "topics": ["aviation"],
                "keywords": ["planes"],
                "summary": "Kevin likes planes."
              },
              "risk_flags": [],
              "evidence_excerpt": "Fred has a plane-shaped sticky note now."
            }
          ]
        }"#,
    )
    .expect("response");

    let partition = partition_memory_extraction_response_with_assistant_items(
        response,
        "Nice",
        &["Tiny but important onboarding victory. Fred has a plane-shaped sticky note now."],
    );

    assert!(partition.accepted.is_empty());
    assert_eq!(partition.rejected.len(), 1);
    assert!(
        partition.rejected[0]
            .error
            .contains("human-subject memories require direct user evidence")
    );
}

#[test]
fn assistant_evidence_can_support_conversation_note() {
    let response: ExtractorMemoryResponse = serde_json::from_str(
        r#"{
          "proposals": [
            {
              "content": "The current conversation has a durable assistant note.",
              "memory_type": "note",
              "title": "Assistant note",
              "confidence": 0.78,
              "sensitivity": "normal",
              "subjects": [
                {
                  "id": null,
                  "kind": "conversation",
                  "name": "current conversation",
                  "role": "about"
                }
              ],
              "retrieval_hints": {
                "topics": ["memory"],
                "keywords": ["assistant note"],
                "summary": "The current conversation has a durable assistant note."
              },
              "risk_flags": [],
              "evidence_excerpt": "Second assistant item contains the durable note."
            }
          ]
        }"#,
    )
    .expect("response");

    let proposals = validate_memory_extraction_response_with_assistant_items(
        response,
        "Emit two assistant notes and save the second.",
        &[
            "First assistant item should not own the evidence.",
            "Second assistant item contains the durable note.",
        ],
    )
    .expect("assistant conversation note");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Candidate);
}

#[test]
fn mislabelled_secret_sensitivity_is_escalated_before_promotion() {
    let user_input = "My API key is sk-testSecretToken123456789.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "Kevin's API key is sk-testSecretToken123456789.",
          "memory_type": "note",
          "title": "API key",
          "confidence": 0.97,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": "human:local",
              "kind": "human",
              "name": "Kevin",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["credentials"],
            "keywords": ["api key"],
            "summary": "Kevin's API key is sk-testSecretToken123456789."
          },
          "risk_flags": [],
          "evidence_excerpt": "My API key is sk-testSecretToken123456789."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].proposal.sensitivity, Sensitivity::Secret);
    assert_eq!(proposals[0].status, MemoryStatus::Candidate);
}

#[test]
fn local_human_name_kevin_promotes_active() {
    let user_input = "I prefer dark mode.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "Kevin prefers dark mode.",
          "memory_type": "preference",
          "title": "Dark mode preference",
          "confidence": 0.92,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": null,
              "kind": "human",
              "name": "Kevin",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["display"],
            "keywords": ["dark mode"],
            "summary": "Kevin prefers dark mode."
          },
          "risk_flags": [],
          "evidence_excerpt": "I prefer dark mode."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Active);
}

#[test]
fn literal_local_human_name_promotes_active() {
    let user_input = "I prefer local models.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "The local human prefers local models.",
          "memory_type": "preference",
          "title": "Local model preference",
          "confidence": 0.92,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": null,
              "kind": "human",
              "name": "local human",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["models"],
            "keywords": ["local models"],
            "summary": "The local human prefers local models."
          },
          "risk_flags": [],
          "evidence_excerpt": "I prefer local models."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Active);
}

#[test]
fn same_name_third_party_human_stays_candidate() {
    let user_input = "My friend Kevin prefers decaf.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "Kevin prefers decaf.",
          "memory_type": "preference",
          "title": "Kevin decaf preference",
          "confidence": 0.91,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": null,
              "kind": "human",
              "name": "Kevin",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["people"],
            "keywords": ["Kevin", "decaf"],
            "summary": null
          },
          "risk_flags": [],
          "evidence_excerpt": "My friend Kevin prefers decaf."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Candidate);
}

#[test]
fn same_name_user_assertion_without_named_kevin_promotes_active() {
    let user_input = "Garage keypad code is 1234.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "Garage keypad code is 1234.",
          "memory_type": "note",
          "title": "Garage code",
          "confidence": 0.91,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": null,
              "kind": "human",
              "name": "Kevin",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["home"],
            "keywords": ["garage"],
            "summary": "Garage keypad code is 1234."
          },
          "risk_flags": [],
          "evidence_excerpt": "Garage keypad code is 1234."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Candidate);
    assert!(memory_extraction_subject_implies_local_human(
        &proposals[0].proposal.subjects[0],
        &proposals[0].proposal.evidence_excerpt
    ));
}

#[test]
fn same_name_named_third_party_human_stays_candidate() {
    let user_input = "I talked to Kevin and he prefers decaf.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "Kevin prefers decaf.",
          "memory_type": "preference",
          "title": "Kevin decaf preference",
          "confidence": 0.91,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": null,
              "kind": "human",
              "name": "Kevin",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["people"],
            "keywords": ["Kevin", "decaf"],
            "summary": null
          },
          "risk_flags": [],
          "evidence_excerpt": "I talked to Kevin and he prefers decaf."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Candidate);
    assert!(!memory_extraction_subject_implies_local_human(
        &proposals[0].proposal.subjects[0],
        &proposals[0].proposal.evidence_excerpt
    ));
}

#[test]
fn sensitive_proposal_stays_candidate() {
    let user_input = "My doctor appointment is on Friday.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "The current human has a doctor appointment on Friday.",
          "memory_type": "fact",
          "title": "Doctor appointment",
          "confidence": 0.95,
          "sensitivity": "sensitive",
          "subjects": [
            {
              "id": "human:local",
              "kind": "human",
              "name": "current human",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["health", "appointment"],
            "keywords": ["doctor", "Friday"],
            "summary": null
          },
          "risk_flags": ["sensitive"],
          "evidence_excerpt": "My doctor appointment is on Friday."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Candidate);
}

#[test]
fn quote_mismatch_is_rejected() {
    let user_input = "I prefer terse summaries.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "The current human prefers terse summaries.",
          "memory_type": "preference",
          "title": null,
          "confidence": 0.91,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": "human:local",
              "kind": "human",
              "name": "current human",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": [],
            "keywords": [],
            "summary": null
          },
          "risk_flags": [],
          "evidence_excerpt": "I like terse summaries."
        }
      ]
    }"#;

    let error = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect_err("mismatched quote should fail");

    assert!(matches!(
        error,
        MemoryExtractionError::InvalidProposal { index: 0, .. }
    ));
}

#[test]
fn malformed_json_is_rejected() {
    let error = parse_memory_extraction_proposals("{not json", "hello", ASSISTANT_RESPONSE)
        .expect_err("malformed JSON should fail");

    assert!(matches!(error, MemoryExtractionError::InvalidJson { .. }));
}

#[test]
fn action_triggering_proposal_stays_candidate() {
    let user_input = "Remind me to check CI every weekday.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "The current human wants to be reminded to check CI every weekday.",
          "memory_type": "preference",
          "title": "CI reminder preference",
          "confidence": 0.92,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": "human:local",
              "kind": "human",
              "name": "current human",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["ci", "reminder"],
            "keywords": ["check CI", "weekday"],
            "summary": null
          },
          "risk_flags": ["action_triggering"],
          "evidence_excerpt": "Remind me to check CI every weekday."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Candidate);
}

#[test]
fn third_party_human_preference_stays_candidate() {
    let user_input = "Alice prefers decaf.";
    let extracted = r#"{
      "proposals": [
        {
          "content": "Alice prefers decaf.",
          "memory_type": "preference",
          "title": "Alice decaf preference",
          "confidence": 0.91,
          "sensitivity": "normal",
          "subjects": [
            {
              "id": null,
              "kind": "human",
              "name": "Alice",
              "role": "about"
            }
          ],
          "retrieval_hints": {
            "topics": ["people"],
            "keywords": ["Alice", "decaf"],
            "summary": null
          },
          "risk_flags": [],
          "evidence_excerpt": "Alice prefers decaf."
        }
      ]
    }"#;

    let proposals = parse_memory_extraction_proposals(extracted, user_input, ASSISTANT_RESPONSE)
        .expect("valid proposal");

    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].status, MemoryStatus::Candidate);
}
