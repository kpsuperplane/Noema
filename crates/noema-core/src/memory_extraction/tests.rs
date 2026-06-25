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
