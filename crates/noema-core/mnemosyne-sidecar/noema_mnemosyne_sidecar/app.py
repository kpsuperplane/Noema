from __future__ import annotations

import ast
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
    source_rows = _memory_source_rows_by_id(memory)
    fact_rows = _mnemosyne_fact_rows(memory)
    filtered = [
        _memory_result_from_mnemosyne_fact(row, source_rows.get(str(row.get("memory_id", ""))))
        for row in fact_rows
        if _source_row_matches_user(source_rows.get(str(row.get("memory_id", ""))), user_id)
    ]
    filtered = [row for row in filtered if row is not None]
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


def _memory_result_from_mnemosyne_fact(
    row: dict[str, Any],
    source_row: Optional[dict[str, Any]],
) -> Optional[dict[str, Any]]:
    fact_text = _displayable_fact_text(row.get("value"))
    if fact_text is None:
        return None

    memory_id = str(row.get("memory_id") or "")
    created_at = _timestamp_to_iso8601(row.get("created_at")) or _timestamp_to_iso8601(
        (source_row or {}).get("timestamp") or (source_row or {}).get("created_at")
    )
    metadata = dict(_metadata_from_row(source_row or {}))
    metadata.update(
        {
            "memoryKind": "fact",
            "mnemosyneMemoryId": memory_id,
        }
    )
    source_content = (source_row or {}).get("content")
    if isinstance(source_content, str) and source_content.strip():
        metadata["sourceObservation"] = source_content.strip()

    fact_id = row.get("id") or row.get("fact_id") or memory_id or fact_text
    result = {
        "id": f"fact:{fact_id}",
        "memory": fact_text,
        "metadata": metadata,
        "created_at": created_at,
        "updated_at": created_at,
    }
    if row.get("confidence") is not None:
        result["score"] = row["confidence"]
    return result


def _mnemosyne_fact_rows(memory: Any) -> list[dict[str, Any]]:
    annotations = getattr(getattr(memory, "beam", None), "annotations", None)
    if annotations is None:
        return []
    rows = annotations.query_by_kind("fact", filter_noise=False)
    return [dict(row) for row in rows]


def _memory_source_rows_by_id(memory: Any) -> dict[str, dict[str, Any]]:
    rows = memory.get_all_memories()
    return {str(row.get("id")): dict(row) for row in rows if row.get("id")}


def _source_row_matches_user(source_row: Optional[dict[str, Any]], user_id: str) -> bool:
    if source_row is None:
        return True
    return _metadata_from_row(source_row).get("user_id", user_id) == user_id


def _displayable_fact_text(value: Any) -> Optional[str]:
    text = _fact_value_to_text(value).strip()
    if not text:
        return None
    if text.lower() in {"facts", "instructions", "preferences", "timelines"}:
        return None
    return text


def _fact_value_to_text(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, dict):
        return _fact_dict_to_text(value)
    if not isinstance(value, str):
        return str(value)

    stripped = value.strip()
    if stripped.startswith("{") and stripped.endswith("}"):
        try:
            parsed = ast.literal_eval(stripped)
        except (SyntaxError, ValueError):
            parsed = None
        if isinstance(parsed, dict):
            return _fact_dict_to_text(parsed)
    return stripped


def _fact_dict_to_text(value: dict[str, Any]) -> str:
    text = value.get("text")
    if isinstance(text, str) and text.strip():
        return text.strip()

    subject = _string_value(value.get("subject"))
    predicate = _string_value(value.get("predicate"))
    obj = _string_value(value.get("object"))
    return " ".join(part for part in (subject, predicate, obj) if part).strip()


def _string_value(value: Any) -> str:
    return value.strip() if isinstance(value, str) else ""


def _timestamp_to_iso8601(value: Any) -> Optional[str]:
    if not isinstance(value, str) or not value.strip():
        return None
    timestamp = value.strip()
    if "T" in timestamp:
        return timestamp
    return timestamp.replace(" ", "T", 1)


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
