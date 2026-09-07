package store

// Migration v36 removed the route flag. Migration v37 restores it without
// changing this already applied migration.
const schemaV36SQL = `ALTER TABLE task_model_pool_settings DROP COLUMN enabled;`
