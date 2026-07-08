from __future__ import annotations

from typing import Any, Literal

from fastapi import FastAPI, HTTPException
from pydantic import BaseModel, Field

from .config import build_memory_from_env


class Message(BaseModel):
    role: Literal["user"]
    content: str = Field(min_length=1)


class AddMemoryRequest(BaseModel):
    messages: list[Message]
    user_id: str
    agent_id: str | None = None
    run_id: str | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)


class SearchMemoryRequest(BaseModel):
    query: str = Field(min_length=1)
    user_id: str
    agent_id: str | None = None
    run_id: str | None = None
    limit: int = Field(default=8, ge=1, le=100)


def create_app(memory: Any | None = None) -> FastAPI:
    app = FastAPI()
    app.state.memory = memory or build_memory_from_env()

    @app.get("/health")
    async def health() -> dict[str, str]:
        return {"status": "ready"}

    @app.post("/v1/memories/add")
    async def add_memory(request: AddMemoryRequest) -> Any:
        try:
            return await app.state.memory.add(
                messages=[message.model_dump() for message in request.messages],
                user_id=request.user_id,
                agent_id=request.agent_id,
                run_id=request.run_id,
                metadata=request.metadata,
            )
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc

    @app.post("/v1/memories/search")
    async def search_memory(request: SearchMemoryRequest) -> Any:
        filters: dict[str, Any] = {"user_id": request.user_id}
        if request.agent_id:
            filters["agent_id"] = request.agent_id
        if request.run_id:
            filters["run_id"] = request.run_id
        try:
            return await app.state.memory.search(
                query=request.query,
                filters=filters,
                top_k=request.limit,
            )
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc

    @app.get("/v1/memories")
    async def list_memories(user_id: str, limit: int = 100) -> Any:
        try:
            return await app.state.memory.get_all(
                filters={"user_id": user_id},
                limit=limit,
            )
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc

    @app.get("/v1/memories/{memory_id}/history")
    async def history(memory_id: str) -> Any:
        try:
            return await app.state.memory.history(memory_id=memory_id)
        except Exception as exc:
            raise HTTPException(status_code=500, detail=str(exc)) from exc

    return app


app = create_app()
