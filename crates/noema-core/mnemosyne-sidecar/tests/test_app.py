from fastapi.testclient import TestClient

from noema_mnemosyne_sidecar.app import create_app


class FakeMnemosyne:
    def __init__(self):
        self.remember_calls = []
        self.recall_calls = []
        self.beam = FakeBeam()

    def remember(
        self,
        *,
        content,
        source="conversation",
        importance=0.5,
        metadata=None,
        valid_until=None,
        scope="session",
        extract_entities=False,
        extract=False,
        trust_tier=None,
    ):
        self.remember_calls.append(
            {
                "content": content,
                "source": source,
                "importance": importance,
                "metadata": metadata,
                "valid_until": valid_until,
                "scope": scope,
                "extract_entities": extract_entities,
                "extract": extract,
                "trust_tier": trust_tier,
            }
        )
        return "mem_1"

    def recall(self, **kwargs):
        self.recall_calls.append(kwargs)
        return [
            {
                "id": "mem_1",
                "content": "The user loves planes.",
                "score": 0.91,
                "metadata": {"noemaConversationId": "conv:1"},
                "timestamp": "2026-07-08T00:00:00",
            }
        ]

    def get_all_memories(self):
        return [
            {
                "id": "mem_1",
                "content": "I love planes.",
                "metadata_json": '{"user_id":"human:local","noemaConversationId":"conv:1","userItemId":"item:1"}',
                "timestamp": "2026-07-08T00:00:00",
            }
        ]


class FakeBeam:
    def __init__(self):
        self.annotations = FakeAnnotations()


class FakeAnnotations:
    def query_by_kind(self, kind, filter_noise=True):
        assert kind == "fact"
        assert filter_noise is False
        return [
            {
                "id": 7,
                "memory_id": "mem_1",
                "kind": "fact",
                "value": "The user loves planes.",
                "source": "conversation",
                "confidence": 0.7,
                "created_at": "2026-07-08 00:00:01",
            },
            {
                "id": 8,
                "memory_id": "mem_1",
                "kind": "fact",
                "value": "preferences",
                "source": "conversation",
                "confidence": 0.7,
                "created_at": "2026-07-08 00:00:02",
            },
        ]


def test_health_reports_ready():
    client = TestClient(create_app(memory_factory=lambda **_: FakeMnemosyne()))

    response = client.get("/health")

    assert response.status_code == 200
    assert response.json() == {"status": "ready"}


def test_add_remembers_user_message_with_extraction_and_noema_metadata():
    memory = FakeMnemosyne()
    client = TestClient(create_app(memory_factory=lambda **_: memory))

    response = client.post(
        "/v1/memories/add",
        json={
            "messages": [{"role": "user", "content": "I love planes."}],
            "user_id": "human:local",
            "agent_id": "agent:local",
            "run_id": "conv:1",
            "metadata": {"noemaConversationId": "conv:1", "userItemId": "item:1"},
        },
    )

    assert response.status_code == 200
    assert response.json()["results"][0]["id"] == "mem_1"
    assert memory.remember_calls == [
        {
            "content": "I love planes.",
            "source": "conversation",
            "importance": 0.7,
            "metadata": {
                "user_id": "human:local",
                "agent_id": "agent:local",
                "run_id": "conv:1",
                "noemaConversationId": "conv:1",
                "userItemId": "item:1",
            },
            "valid_until": None,
            "scope": "global",
            "extract_entities": True,
            "extract": True,
            "trust_tier": "STATED",
        }
    ]


def test_search_uses_recall_with_author_and_channel_filters():
    memory = FakeMnemosyne()
    client = TestClient(create_app(memory_factory=lambda **_: memory))

    response = client.post(
        "/v1/memories/search",
        json={
            "query": "planes",
            "user_id": "human:local",
            "run_id": "conv:1",
            "limit": 7,
        },
    )

    assert response.status_code == 200
    assert response.json()["results"][0]["memory"] == "The user loves planes."
    assert memory.recall_calls == [
        {
            "query": "planes",
            "top_k": 7,
            "author_id": "human:local",
            "author_type": "human",
            "channel_id": "conv:1",
        }
    ]


def test_list_memories_maps_extracted_facts_to_noema_contract():
    client = TestClient(create_app(memory_factory=lambda **_: FakeMnemosyne()))

    response = client.get("/v1/memories", params={"user_id": "human:local", "limit": 7})

    assert response.status_code == 200
    assert response.json() == {
        "results": [
            {
                "id": "fact:7",
                "memory": "The user loves planes.",
                "metadata": {
                    "user_id": "human:local",
                    "noemaConversationId": "conv:1",
                    "userItemId": "item:1",
                    "memoryKind": "fact",
                    "mnemosyneMemoryId": "mem_1",
                    "sourceObservation": "I love planes.",
                },
                "created_at": "2026-07-08T00:00:01",
                "updated_at": "2026-07-08T00:00:01",
                "score": 0.7,
            }
        ]
    }
