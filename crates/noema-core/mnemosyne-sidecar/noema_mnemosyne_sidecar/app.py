from __future__ import annotations

import json
from typing import Any, Callable, Literal, Optional

import anyio
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

from .config import build_memory_factory_from_env


class Message(BaseModel):
    role: Literal["user"]
    content: str = Field(min_length=1)


class AddMemoryRequest(BaseModel):
    messages: list[Message]
    user_id: str
    agent_id: Optional[str] = None
    run_id: Optional[str] = None
    metadata: dict[str, Any] = Field(default_factory=dict)


class SearchMemoryRequest(BaseModel):
    query: str = Field(min_length=1)
    user_id: str
    agent_id: Optional[str] = None
    run_id: Optional[str] = None
    limit: int = Field(default=8, ge=1, le=100)


MemoryFactory = Callable[..., Any]


def create_app(memory_factory: Optional[MemoryFactory] = None) -> FastAPI:
    app = FastAPI()
    app.state.memory_factory = memory_factory or build_memory_factory_from_env()

    @app.get("/health")
    async def health() -> dict[str, str]:
        return {"status": "ready"}

    @app.post("/v1/memories/add")
    async def add_memory(request: AddMemoryRequest) -> dict[str, list[dict[str, str]]]:
        try:
            memory_id = await anyio.to_thread.run_sync(_remember_observation, app, request)
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc
        return {"results": [{"id": memory_id}]}

    @app.post("/v1/memories/search")
    async def search_memory(request: SearchMemoryRequest) -> dict[str, list[dict[str, Any]]]:
        try:
            results = await anyio.to_thread.run_sync(_search_memories, app, request)
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc
        return {"results": results}

    @app.get("/v1/memories")
    async def list_memories(user_id: str, limit: int = 100) -> dict[str, list[dict[str, Any]]]:
        try:
            results = await anyio.to_thread.run_sync(_list_memories, app, user_id, limit)
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc
        return {"results": results}

    return app


def _memory(app: FastAPI, *, user_id: str, run_id: Optional[str], agent_id: Optional[str]) -> Any:
    return app.state.memory_factory(user_id=user_id, run_id=run_id, agent_id=agent_id)


def _remember_observation(app: FastAPI, request: AddMemoryRequest) -> str:
    content = "\n".join(
        message.content.strip() for message in request.messages if message.content.strip()
    )
    if not content:
        raise ValueError("memory observation content is empty")

    metadata = {
        "user_id": request.user_id,
        "agent_id": request.agent_id,
        "run_id": request.run_id,
        **request.metadata,
    }
    memory = _memory(
        app,
        user_id=request.user_id,
        run_id=request.run_id,
        agent_id=request.agent_id,
    )
    return memory.remember(
        content=content,
        source="conversation",
        importance=0.7,
        metadata=metadata,
        scope="global",
        extract_entities=True,
        extract=True,
        trust_tier="STATED",
    )


def _search_memories(app: FastAPI, request: SearchMemoryRequest) -> list[dict[str, Any]]:
    memory = _memory(
        app,
        user_id=request.user_id,
        run_id=request.run_id,
        agent_id=request.agent_id,
    )
    results = memory.recall(
        query=request.query,
        top_k=request.limit,
        author_id=request.user_id,
        author_type="human",
        channel_id=request.run_id,
    )
    return [_memory_result_from_mnemosyne(result) for result in results[: request.limit]]


def _list_memories(app: FastAPI, user_id: str, limit: int) -> list[dict[str, Any]]:
    memory = _memory(app, user_id=user_id, run_id=None, agent_id=None)
    rows = memory.get_all_memories()
    filtered = [
        _memory_result_from_mnemosyne(row)
        for row in rows
        if _metadata_from_row(row).get("user_id", user_id) == user_id
    ]
    filtered.sort(key=lambda row: row.get("updated_at") or "", reverse=True)
    return filtered[:limit]


def _memory_result_from_mnemosyne(row: dict[str, Any]) -> dict[str, Any]:
    timestamp = row.get("timestamp") or row.get("created_at")
    result = {
        "id": str(row.get("id", "")),
        "memory": row.get("content") or row.get("memory"),
        "metadata": _metadata_from_row(row),
        "created_at": timestamp,
        "updated_at": timestamp,
    }
    if row.get("score") is not None:
        result["score"] = row["score"]
    return result


def _metadata_from_row(row: dict[str, Any]) -> dict[str, Any]:
    metadata = row.get("metadata")
    if isinstance(metadata, dict):
        return metadata
    metadata_json = row.get("metadata_json")
    if isinstance(metadata_json, str) and metadata_json.strip():
        try:
            parsed = json.loads(metadata_json)
            if isinstance(parsed, dict):
                return parsed
        except json.JSONDecodeError:
            return {}
    return {}
