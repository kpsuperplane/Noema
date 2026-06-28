use ring::digest;

use super::{
    helpers::{participant_role_to_db, sensitivity_to_db, subject_role_to_db},
    models::NewMemoryCandidate,
};

pub(crate) fn memory_candidate_dedupe_fingerprint(candidate: &NewMemoryCandidate) -> String {
    let mut participants = candidate
        .participants
        .iter()
        .map(|participant| {
            format!(
                "{}:{}",
                participant.participant.actor_id,
                participant_role_to_db(participant.role)
            )
        })
        .collect::<Vec<_>>();
    participants.sort();

    let mut subjects = candidate
        .subjects
        .iter()
        .map(|subject| {
            format!(
                "{}:{}:{}",
                subject.entity_id,
                subject.entity_type,
                subject_role_to_db(subject.role)
            )
        })
        .collect::<Vec<_>>();
    subjects.sort();

    let canonical = serde_json::json!({
        "owner": {
            "object_type": candidate.owner.object_type.as_str(),
            "object_id": candidate.owner.object_id.as_str(),
        },
        "memory_type": candidate.memory_type.as_str(),
        "content": canonical_content(&candidate.content),
        "sensitivity": sensitivity_to_db(candidate.sensitivity),
        "participants": participants,
        "subjects": subjects,
    });
    let bytes = serde_json::to_vec(&canonical).expect("canonical dedupe JSON is serializable");
    let digest = digest::digest(&digest::SHA256, &bytes);
    format!("sha256:{}", hex_lower(digest.as_ref()))
}

fn canonical_content(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .trim_end_matches(['.', '!', '?'])
        .trim()
        .to_ascii_lowercase()
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::{
        memory::{ParticipantRole, Sensitivity, SubjectRole},
        memory_persistence::{
            ActorRef, MemoryExtractionMethod, MemoryType, NewMemoryCandidate, NewMemoryParticipant,
            NewMemorySubject, ObjectRef, ObjectType,
        },
    };

    use super::{canonical_content, memory_candidate_dedupe_fingerprint};

    fn candidate(content: &str) -> NewMemoryCandidate {
        let mut candidate = NewMemoryCandidate::confirmed_note(
            ObjectRef::new(ObjectType::Conversation, "conversation_1").expect("owner"),
            content,
            ActorRef::agent("agent:primary"),
            ObjectRef::conversation_item("item_1"),
        );
        candidate.memory_type = MemoryType::Preference;
        candidate.sensitivity = Sensitivity::Normal;
        candidate.extraction_method = MemoryExtractionMethod::LlmExtracted;
        candidate.participants = vec![
            NewMemoryParticipant::new(
                ActorRef::human("human:local"),
                ParticipantRole::HumanInScope,
            ),
            NewMemoryParticipant::new(
                ActorRef::agent("agent:primary"),
                ParticipantRole::AgentInScope,
            ),
        ];
        let mut subject =
            NewMemorySubject::new("human:local", "human", "Local human", SubjectRole::About);
        subject.linked_object = Some(ObjectRef::human("human:local"));
        candidate.subjects = vec![subject];
        candidate
    }

    #[test]
    fn canonical_content_normalizes_low_risk_surface_changes() {
        assert_eq!(
            canonical_content("  I   Like ICE CREAM!  "),
            "i like ice cream"
        );
        assert_eq!(canonical_content("I like ice cream !"), "i like ice cream");
    }

    #[test]
    fn fingerprint_ignores_source_title_confidence_and_metadata() {
        let mut first = candidate("I like ice cream.");
        first.title = Some("Ice cream".to_string());
        first.confidence = Some(0.95);
        first.metadata = serde_json::json!({"turn_id": "turn_1"});

        let mut second = candidate("  I LIKE   ICE CREAM ");
        second.title = Some("Dessert preference".to_string());
        second.confidence = Some(0.72);
        second.metadata = serde_json::json!({"turn_id": "turn_2"});
        second.source = Some(crate::memory_persistence::ObjectProvenanceSource {
            source: ObjectRef::conversation_item("item_2"),
            evidence_excerpt: Some("I LIKE ICE CREAM".to_string()),
        });

        assert_eq!(
            memory_candidate_dedupe_fingerprint(&first),
            memory_candidate_dedupe_fingerprint(&second)
        );
    }

    #[test]
    fn fingerprint_changes_when_subject_changes() {
        let first = candidate("I like ice cream.");
        let mut second = candidate("I like ice cream.");
        second.subjects = vec![NewMemorySubject::new(
            "person:casey",
            "person",
            "Casey",
            SubjectRole::About,
        )];

        assert_ne!(
            memory_candidate_dedupe_fingerprint(&first),
            memory_candidate_dedupe_fingerprint(&second)
        );
    }
}
