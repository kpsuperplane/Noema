package runtime

import (
	"encoding/json"
	"testing"
)

func TestActionReviewerAcceptsOnlyClosedAssessmentValues(t *testing.T) {
	valid := json.RawMessage(`{"authorization":"explicit","risk":"medium","reason_codes":["action_matches_request"],"explanation":"The request authorizes this action."}`)
	assessment, err := parseActionAssessment(valid)
	if err != nil || assessment.Status != "completed" || assessment.Authorization != "explicit" || assessment.Risk != "medium" {
		t.Fatalf("valid assessment = %#v, %v", assessment, err)
	}
	for _, raw := range []string{
		`{"authorization":"invented","risk":"low","reason_codes":[],"explanation":"No."}`,
		`{"authorization":"explicit","risk":"minor","reason_codes":[],"explanation":"No."}`,
		`{"authorization":"explicit","risk":"low","reason_codes":["invented"],"explanation":"No."}`,
		`{"authorization":"explicit","risk":"low","reason_codes":[],"explanation":"","extra":true}`,
	} {
		if _, err := parseActionAssessment(json.RawMessage(raw)); err == nil {
			t.Fatalf("invalid assessment accepted: %s", raw)
		}
	}
}
