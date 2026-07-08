from __future__ import annotations

import os
from pathlib import Path

from mem0 import AsyncMemory
from mem0.configs.base import MemoryConfig


def build_memory_from_env() -> AsyncMemory:
    data_dir = Path(os.environ["NOEMA_MEM0_DATA_DIR"])
    data_dir.mkdir(parents=True, exist_ok=True)
    base_url = os.environ["NOEMA_MEMORY_OPENAI_BASE_URL"].rstrip("/")
    api_key = os.environ["NOEMA_MEMORY_OPENAI_API_KEY"]
    model = os.environ["NOEMA_MEMORY_MODEL"]

    config = MemoryConfig(
        llm={
            "provider": "openai",
            "config": {
                "api_key": api_key,
                "openai_base_url": base_url,
                "model": model,
            },
        },
        embedder={
            "provider": "fastembed",
            "config": {
                "model": "BAAI/bge-small-en-v1.5",
            },
        },
        vector_store={
            "provider": "chroma",
            "config": {
                "collection_name": "noema_human_local",
                "path": str(data_dir / "chroma"),
            },
        },
    )
    return AsyncMemory(config=config)
