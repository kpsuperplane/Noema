package store

const schemaV37SQL = `ALTER TABLE task_model_pool_settings ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1));`
