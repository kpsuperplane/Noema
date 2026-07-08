from fastapi.testclient import TestClient

from noema_mem0_sidecar.app import create_app


class FakeMemory:
    def __init__(self):
        self.add_calls = []
        self.get_all_calls = []

    async def add(self, **kwargs):
        self.add_calls.append(kwargs)
        return {"results": [{"id": "mem_1", "memory": "The user likes planes."}]}

    async def search(self, **kwargs):
        return {
            "results": [
                {"id": "mem_1", "memory": "The user likes planes.", "score": 0.91}
            ]
        }

    async def get_all(self, **kwargs):
        self.get_all_calls.append(kwargs)
        return {"results": [{"id": "mem_1", "memory": "The user likes planes."}]}

    async def history(self, memory_id):
        return {"memory_id": memory_id, "history": []}


def test_health_reports_ready():
    client = TestClient(create_app(memory=FakeMemory()))

    response = client.get("/health")

    assert response.status_code == 200
    assert response.json() == {"status": "ready"}


def test_add_submits_user_message_with_noema_metadata():
    memory = FakeMemory()
    client = TestClient(create_app(memory=memory))

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
    assert memory.add_calls == [
        {
            "messages": [{"role": "user", "content": "I love planes."}],
            "user_id": "human:local",
            "agent_id": "agent:local",
            "run_id": "conv:1",
            "metadata": {"noemaConversationId": "conv:1", "userItemId": "item:1"},
        }
    ]


def test_list_memories_uses_mem0_top_k_parameter():
    memory = FakeMemory()
    client = TestClient(create_app(memory=memory))

    response = client.get("/v1/memories", params={"user_id": "human:local", "limit": 7})

    assert response.status_code == 200
    assert response.json()["results"][0]["id"] == "mem_1"
    assert memory.get_all_calls == [
        {"filters": {"user_id": "human:local"}, "top_k": 7}
    ]


def test_spacy_entity_dependencies_are_available():
    import spacy
    from mem0.utils.entity_extraction import extract_entities

    assert spacy.util.is_package("en_core_web_sm")
    entities = extract_entities("Alice works at Acme Corp on the Q1 roadmap.")

    assert ("PROPER", "Acme Corp") in entities
