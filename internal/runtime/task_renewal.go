package runtime

import "time"

// claimRenewalEvidence is the complete timing and cancellation snapshot for a
// Task run lease renewal.
type claimRenewalEvidence struct {
	RunID                    string
	PlannedAtUnixMs          int64
	StartedAtUnixMs          int64
	StartDelay               time.Duration
	SQLiteDuration           time.Duration
	TimeBeforeExpiry         time.Duration
	ShutdownRequested        bool
	RunCancellationRequested bool
	ActivePhase              string
}

type claimRenewalEventRecord struct {
	Category string
	Context  map[string]any
}

// claimRenewalEvent builds the runtime authority's exact diagnostic payload.
// The caller supplies the optional storage error so delayed and failed lease
// renewals use the same eight timing and state fields.
func claimRenewalEvent(evidence claimRenewalEvidence, storageErr error) claimRenewalEventRecord {
	category := "task_run_claim_renewal_delayed"
	if storageErr != nil {
		category = "task_run_claim_renewal_failed"
	}
	return claimRenewalEventRecord{
		Category: category,
		Context: map[string]any{
			"run_id":                     evidence.RunID,
			"planned_at_unix_ms":         evidence.PlannedAtUnixMs,
			"started_at_unix_ms":         evidence.StartedAtUnixMs,
			"start_delay_ms":             evidence.StartDelay.Milliseconds(),
			"sqlite_duration_ms":         evidence.SQLiteDuration.Milliseconds(),
			"time_before_expiry_ms":      evidence.TimeBeforeExpiry.Milliseconds(),
			"shutdown_requested":         evidence.ShutdownRequested,
			"run_cancellation_requested": evidence.RunCancellationRequested,
			"active_phase":               evidence.ActivePhase,
		},
	}
}
