from __future__ import annotations

import os
from pathlib import Path
from typing import Any, Callable


def configure_mnemosyne_env() -> Path:
    data_dir = Path(os.environ["NOEMA_MNEMOSYNE_DATA_DIR"])
    data_dir.mkdir(parents=True, exist_ok=True)

    os.environ["MNEMOSYNE_DATA_DIR"] = str(data_dir)
    os.environ.setdefault("MNEMOSYNE_EMBEDDING_MODEL", "BAAI/bge-small-en-v1.5")
    os.environ.setdefault("MNEMOSYNE_VEC_TYPE", "int8")
    os.environ.setdefault("MNEMOSYNE_POLYPHONIC_RECALL", "1")
    os.environ.setdefault("MNEMOSYNE_ENHANCED_RECALL", "1")
    os.environ.setdefault("MNEMOSYNE_FACT_RECALL_ENABLED", "1")

    base_url = os.environ.get("NOEMA_MEMORY_OPENAI_BASE_URL", "").rstrip("/")
    api_key = os.environ.get("NOEMA_MEMORY_OPENAI_API_KEY", "")
    model = os.environ.get("NOEMA_MEMORY_MODEL", "")
    if base_url and model:
        os.environ["MNEMOSYNE_LLM_BASE_URL"] = base_url
        os.environ["MNEMOSYNE_LLM_API_KEY"] = api_key
        os.environ["MNEMOSYNE_LLM_MODEL"] = model
        os.environ.setdefault("MNEMOSYNE_LLM_TIMEOUT", "120")

    return data_dir


def build_memory_factory_from_env() -> Callable[..., Any]:
    data_dir = configure_mnemosyne_env()

    from mnemosyne import Mnemosyne

    db_path = data_dir / "mnemosyne.db"

    def factory(
        *,
        user_id: str,
        run_id: str | None = None,
        agent_id: str | None = None,
    ) -> Any:
        return Mnemosyne(
            session_id=user_id,
            db_path=db_path,
            author_id=user_id,
            author_type="human",
            channel_id=run_id or agent_id or user_id,
        )

    return factory
