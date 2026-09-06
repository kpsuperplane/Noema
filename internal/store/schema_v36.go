package store

// Task models use provider availability without a separate enabled setting.
const schemaV36SQL = `ALTER TABLE task_model_pool_settings DROP COLUMN enabled;`
