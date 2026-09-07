package main

import (
	"encoding/json"
	"net/http"
	"sort"
	"strconv"
	"strings"
	"time"
)

const calendarFixtureVersion = "2026-09-07-calendar-v3-v1"

type calendarPoint struct {
	DateTime string `json:"dateTime,omitempty"`
	Date     string `json:"date,omitempty"`
	TimeZone string `json:"timeZone,omitempty"`
}

type calendarEvent struct {
	Kind           string           `json:"kind"`
	ETag           string           `json:"etag"`
	ID             string           `json:"id"`
	Status         string           `json:"status"`
	HTMLLink       string           `json:"htmlLink"`
	Created        string           `json:"created"`
	Updated        string           `json:"updated"`
	Summary        string           `json:"summary"`
	Description    string           `json:"description,omitempty"`
	Location       string           `json:"location,omitempty"`
	Start          calendarPoint    `json:"start"`
	End            calendarPoint    `json:"end"`
	Recurrence     []string         `json:"recurrence,omitempty"`
	RecurringEvent string           `json:"recurringEventId,omitempty"`
	Organizer      map[string]any   `json:"organizer,omitempty"`
	Attendees      []map[string]any `json:"attendees,omitempty"`
}

func calendarSeed() map[string][]calendarEvent {
	return map[string][]calendarEvent{
		"account-a": {
			calendarEvent{Kind: "calendar#event", ETag: "\"cal-a-001\"", ID: "cal-a-001", Status: "confirmed", HTMLLink: "https://calendar.google.test/event/cal-a-001", Created: "2026-09-01T08:00:00Z", Updated: "2026-09-05T08:00:00Z", Summary: "Daily planning", Description: "Protect the 16:00 stop time.", Start: calendarPoint{DateTime: "2026-09-07T09:00:00-07:00", TimeZone: "America/Los_Angeles"}, End: calendarPoint{DateTime: "2026-09-07T09:30:00-07:00", TimeZone: "America/Los_Angeles"}},
			calendarEvent{Kind: "calendar#event", ETag: "\"cal-a-002\"", ID: "cal-a-002", Status: "confirmed", HTMLLink: "https://calendar.google.test/event/cal-a-002", Created: "2026-09-01T08:00:00Z", Updated: "2026-09-05T08:00:00Z", Summary: "Client launch review", Description: "Bring the launch decision notes.", Location: "Remote", Start: calendarPoint{DateTime: "2026-09-07T13:00:00-07:00", TimeZone: "America/Los_Angeles"}, End: calendarPoint{DateTime: "2026-09-07T14:00:00-07:00", TimeZone: "America/Los_Angeles"}, Attendees: []map[string]any{{"email": "client@example.test", "responseStatus": "accepted"}}},
			calendarEvent{Kind: "calendar#event", ETag: "\"cal-a-003\"", ID: "cal-a-003", Status: "confirmed", HTMLLink: "https://calendar.google.test/event/cal-a-003", Created: "2026-09-01T08:00:00Z", Updated: "2026-09-05T08:00:00Z", Summary: "Personal leave", Start: calendarPoint{Date: "2026-09-08"}, End: calendarPoint{Date: "2026-09-09"}},
			calendarEvent{Kind: "calendar#event", ETag: "\"cal-a-004\"", ID: "cal-a-004", Status: "confirmed", HTMLLink: "https://calendar.google.test/event/cal-a-004", Created: "2026-09-01T08:00:00Z", Updated: "2026-09-05T08:00:00Z", Summary: "Weekly review", Recurrence: []string{"RRULE:FREQ=WEEKLY;BYDAY=FR"}, Start: calendarPoint{DateTime: "2026-09-11T15:00:00-07:00", TimeZone: "America/Los_Angeles"}, End: calendarPoint{DateTime: "2026-09-11T16:00:00-07:00", TimeZone: "America/Los_Angeles"}},
		},
		"account-b": {
			calendarEvent{Kind: "calendar#event", ETag: "\"cal-b-001\"", ID: "cal-b-001", Status: "confirmed", HTMLLink: "https://calendar.google.test/event/cal-b-001", Created: "2026-09-01T08:00:00Z", Updated: "2026-09-05T08:00:00Z", Summary: "Private account meeting", Start: calendarPoint{DateTime: "2026-09-07T10:00:00-04:00", TimeZone: "America/New_York"}, End: calendarPoint{DateTime: "2026-09-07T11:00:00-04:00", TimeZone: "America/New_York"}},
		},
	}
}

func (f *fixture) calendarEvents(account string) []calendarEvent {
	f.mu.Lock()
	defer f.mu.Unlock()
	if f.calendar == nil {
		f.calendar = calendarSeed()
	}
	return append([]calendarEvent(nil), f.calendar[account]...)
}

func (f *fixture) calendarAPI(w http.ResponseWriter, r *http.Request) {
	if !strings.HasPrefix(r.URL.Path, "/calendar/v3/calendars/") {
		return
	}
	account := f.requestAccount(r)
	if account != "account-a" && account != "account-b" {
		f.trace(r, http.StatusUnauthorized)
		w.Header().Set("WWW-Authenticate", `Bearer realm="calendar-fixture", scope="https://www.googleapis.com/auth/calendar.readonly"`)
		writeJSON(w, http.StatusUnauthorized, calendarError(http.StatusUnauthorized, "Login Required", "authError"))
		return
	}
	path := strings.TrimPrefix(r.URL.Path, "/calendar/v3/calendars/")
	parts := strings.Split(strings.Trim(path, "/"), "/")
	if len(parts) < 2 || parts[0] != "primary" || parts[1] != "events" || len(parts) > 3 {
		f.trace(r, http.StatusNotFound)
		writeJSON(w, http.StatusNotFound, calendarError(http.StatusNotFound, "Calendar not found", "notFound"))
		return
	}
	switch {
	case r.Method == http.MethodGet && len(parts) == 2:
		f.calendarList(w, r, account)
	case r.Method == http.MethodGet && len(parts) == 3:
		f.calendarGet(w, r, account, parts[2])
	case r.Method == http.MethodPost && len(parts) == 2:
		f.calendarInsert(w, r, account)
	case r.Method == http.MethodPatch && len(parts) == 3:
		f.calendarUpdate(w, r, account, parts[2])
	case r.Method == http.MethodDelete && len(parts) == 3:
		f.calendarDelete(w, r, account, parts[2])
	default:
		f.trace(r, http.StatusMethodNotAllowed)
		writeJSON(w, http.StatusMethodNotAllowed, calendarError(http.StatusMethodNotAllowed, "Method Not Allowed", "methodNotAllowed"))
	}
}

func (f *fixture) calendarList(w http.ResponseWriter, r *http.Request, account string) {
	query := strings.ToLower(strings.TrimSpace(r.URL.Query().Get("q")))
	minTime := calendarBound(r.URL.Query().Get("timeMin"), time.Time{})
	maxTime := calendarBound(r.URL.Query().Get("timeMax"), time.Date(9999, 12, 31, 0, 0, 0, 0, time.UTC))
	matches := make([]calendarEvent, 0)
	for _, event := range f.calendarEvents(account) {
		if query != "" && !strings.Contains(strings.ToLower(event.Summary+" "+event.Description), query) {
			continue
		}
		start, end := calendarEventBounds(event)
		if !end.After(minTime) || !start.Before(maxTime) {
			continue
		}
		matches = append(matches, event)
	}
	sort.SliceStable(matches, func(i, j int) bool { return calendarEventStart(matches[i]).Before(calendarEventStart(matches[j])) })
	page := 0
	switch r.URL.Query().Get("pageToken") {
	case "":
	case "page-2":
		page = 1
	case "page-3":
		page = 2
	default:
		f.trace(r, http.StatusBadRequest)
		writeJSON(w, http.StatusBadRequest, calendarError(http.StatusBadRequest, "Invalid page token", "invalid"))
		return
	}
	pageSize := 2
	if raw := r.URL.Query().Get("maxResults"); raw != "" {
		if value, err := strconv.Atoi(raw); err != nil || value < 1 || value > 2500 {
			f.trace(r, http.StatusBadRequest)
			writeJSON(w, http.StatusBadRequest, calendarError(http.StatusBadRequest, "Invalid maxResults", "invalid"))
			return
		}
		if value, _ := strconv.Atoi(raw); value < pageSize {
			pageSize = value
		}
	}
	start := page * pageSize
	if start > len(matches) {
		start = len(matches)
	}
	end := min(start+pageSize, len(matches))
	result := map[string]any{"kind": "calendar#events", "etag": "\"calendar-" + account + "\"", "summary": "Primary", "timeZone": "America/Los_Angeles", "items": matches[start:end]}
	if end < len(matches) {
		result["nextPageToken"] = "page-" + strconv.Itoa(page+2)
	}
	f.trace(r, http.StatusOK)
	writeJSON(w, http.StatusOK, result)
}

func (f *fixture) calendarGet(w http.ResponseWriter, r *http.Request, account, id string) {
	for _, event := range f.calendarEvents(account) {
		if event.ID == id {
			f.trace(r, http.StatusOK)
			writeJSON(w, http.StatusOK, event)
			return
		}
	}
	f.trace(r, http.StatusNotFound)
	writeJSON(w, http.StatusNotFound, calendarError(http.StatusNotFound, "Event not found", "notFound"))
}

func (f *fixture) calendarInsert(w http.ResponseWriter, r *http.Request, account string) {
	r.Body = http.MaxBytesReader(w, r.Body, 64<<10)
	var input struct {
		Summary     string        `json:"summary"`
		Description string        `json:"description"`
		Location    string        `json:"location"`
		Start       calendarPoint `json:"start"`
		End         calendarPoint `json:"end"`
	}
	if json.NewDecoder(r.Body).Decode(&input) != nil || strings.TrimSpace(input.Summary) == "" || !calendarPointValid(input.Start) || !calendarPointValid(input.End) {
		f.trace(r, http.StatusBadRequest)
		writeJSON(w, http.StatusBadRequest, calendarError(http.StatusBadRequest, "Event requires summary, start, and end", "required"))
		return
	}
	f.mu.Lock()
	if f.calendar == nil {
		f.calendar = calendarSeed()
	}
	id := account + "-created-" + strconv.Itoa(len(f.calendar[account])+1)
	now := time.Now().UTC().Format(time.RFC3339Nano)
	event := calendarEvent{Kind: "calendar#event", ETag: "\"" + id + "\"", ID: id, Status: "confirmed", HTMLLink: "https://calendar.google.test/event/" + id, Created: now, Updated: now, Summary: input.Summary, Description: input.Description, Location: input.Location, Start: input.Start, End: input.End}
	f.calendar[account] = append(f.calendar[account], event)
	f.mu.Unlock()
	f.trace(r, http.StatusCreated)
	writeJSON(w, http.StatusCreated, event)
}

func (f *fixture) calendarUpdate(w http.ResponseWriter, r *http.Request, account, id string) {
	r.Body = http.MaxBytesReader(w, r.Body, 64<<10)
	var input map[string]json.RawMessage
	if json.NewDecoder(r.Body).Decode(&input) != nil {
		f.trace(r, http.StatusBadRequest)
		writeJSON(w, http.StatusBadRequest, calendarError(http.StatusBadRequest, "Invalid event", "invalid"))
		return
	}
	f.mu.Lock()
	for index := range f.calendarEventsLocked(account) {
		if f.calendar[account][index].ID != id {
			continue
		}
		if raw := input["summary"]; raw != nil {
			if json.Unmarshal(raw, &f.calendar[account][index].Summary) != nil || strings.TrimSpace(f.calendar[account][index].Summary) == "" {
				f.mu.Unlock()
				f.trace(r, http.StatusBadRequest)
				writeJSON(w, http.StatusBadRequest, calendarError(http.StatusBadRequest, "Invalid summary", "invalid"))
				return
			}
		}
		if raw := input["start"]; raw != nil && (json.Unmarshal(raw, &f.calendar[account][index].Start) != nil || !calendarPointValid(f.calendar[account][index].Start)) {
			f.mu.Unlock()
			f.trace(r, http.StatusBadRequest)
			writeJSON(w, http.StatusBadRequest, calendarError(http.StatusBadRequest, "Invalid start", "invalid"))
			return
		}
		if raw := input["end"]; raw != nil && (json.Unmarshal(raw, &f.calendar[account][index].End) != nil || !calendarPointValid(f.calendar[account][index].End)) {
			f.mu.Unlock()
			f.trace(r, http.StatusBadRequest)
			writeJSON(w, http.StatusBadRequest, calendarError(http.StatusBadRequest, "Invalid end", "invalid"))
			return
		}
		if raw := input["description"]; raw != nil {
			_ = json.Unmarshal(raw, &f.calendar[account][index].Description)
		}
		if raw := input["location"]; raw != nil {
			_ = json.Unmarshal(raw, &f.calendar[account][index].Location)
		}
		f.calendar[account][index].Updated = time.Now().UTC().Format(time.RFC3339Nano)
		f.calendar[account][index].ETag = "\"" + id + "-updated\""
		event := f.calendar[account][index]
		f.mu.Unlock()
		f.trace(r, http.StatusOK)
		writeJSON(w, http.StatusOK, event)
		return
	}
	f.mu.Unlock()
	f.trace(r, http.StatusNotFound)
	writeJSON(w, http.StatusNotFound, calendarError(http.StatusNotFound, "Event not found", "notFound"))
}

func (f *fixture) calendarDelete(w http.ResponseWriter, r *http.Request, account, id string) {
	f.mu.Lock()
	for index := range f.calendarEventsLocked(account) {
		if f.calendar[account][index].ID == id {
			f.calendar[account] = append(f.calendar[account][:index], f.calendar[account][index+1:]...)
			f.mu.Unlock()
			f.trace(r, http.StatusNoContent)
			w.WriteHeader(http.StatusNoContent)
			return
		}
	}
	f.mu.Unlock()
	f.trace(r, http.StatusNotFound)
	writeJSON(w, http.StatusNotFound, calendarError(http.StatusNotFound, "Event not found", "notFound"))
}

func (f *fixture) calendarEventsLocked(account string) []calendarEvent {
	if f.calendar == nil {
		f.calendar = calendarSeed()
	}
	return f.calendar[account]
}

func calendarPointValid(point calendarPoint) bool {
	if point.Date != "" {
		_, err := time.Parse("2006-01-02", point.Date)
		return err == nil
	}
	if point.DateTime == "" {
		return false
	}
	_, err := time.Parse(time.RFC3339, point.DateTime)
	return err == nil
}

func calendarEventStart(event calendarEvent) time.Time {
	if event.Start.Date != "" {
		value, _ := time.Parse("2006-01-02", event.Start.Date)
		return value.UTC()
	}
	value, _ := time.Parse(time.RFC3339, event.Start.DateTime)
	return value
}

func calendarEventBounds(event calendarEvent) (time.Time, time.Time) {
	start := calendarEventStart(event)
	if event.End.Date != "" {
		value, _ := time.Parse("2006-01-02", event.End.Date)
		return start, value.UTC()
	}
	value, _ := time.Parse(time.RFC3339, event.End.DateTime)
	return start, value
}

func calendarBound(raw string, fallback time.Time) time.Time {
	if raw == "" {
		return fallback
	}
	value, err := time.Parse(time.RFC3339, raw)
	if err != nil {
		return fallback
	}
	return value
}

func calendarError(code int, message, reason string) map[string]any {
	return map[string]any{"error": map[string]any{"code": code, "message": message, "errors": []map[string]string{{"domain": "global", "reason": reason, "message": message}}}}
}

var calendarDocs = `# Google Calendar API fixture

Contract: ` + calendarFixtureVersion + ` (Google Calendar v3-shaped read/write contract).

This service exposes a deterministic, synthetic subset of the Google Calendar
v3 REST API. It never contacts Google or changes a real calendar.

Official references:

- https://developers.google.com/calendar/api/v3/reference/events/list
- https://developers.google.com/calendar/api/v3/reference/events/get
- https://developers.google.com/calendar/api/v3/reference/events/insert
- https://developers.google.com/calendar/api/v3/reference/events/patch
- https://developers.google.com/calendar/api/v3/reference/events/delete

## Base URL and authentication

The base URL is ` + "`https://noema.kevinpei.com/__pa-replay/calendar/v3`" + `.
Every request needs ` + "`Authorization: Bearer <access token>`" + `. The operator
supplies the synthetic account token through Noema's protected setup field.
The read-only scope is ` + "`https://www.googleapis.com/auth/calendar.readonly`" + `;
write operations require ` + "`https://www.googleapis.com/auth/calendar`" + `.
The fixture accepts the two synthetic account tokens. Do not put credentials in
Chat, task documents, or evidence.

## Operations

` + "`GET /calendars/{calendarId}/events`" + ` lists events. Use ` + "`primary`" + ` as
the calendar ID. Supported query fields are ` + "`q`" + `, ` + "`timeMin`" + `,
` + "`timeMax`" + `, ` + "`singleEvents`" + `, ` + "`orderBy`" + `, ` + "`pageToken`" + `,
and ` + "`maxResults`" + `. Results use Google's ` + "`items`" + ` and opaque
` + "`nextPageToken`" + ` fields. Event times preserve RFC3339 offsets and
date-only all-day values. Recurring events retain their ` + "`recurrence`" + `.

` + "`GET /calendars/{calendarId}/events/{eventId}`" + ` returns one event.
` + "`POST /calendars/{calendarId}/events`" + ` creates one event after approval.
The JSON body needs ` + "`summary`" + `, ` + "`start`" + `, and ` + "`end`" + `.
` + "`PATCH /calendars/{calendarId}/events/{eventId}`" + ` updates supplied fields.
` + "`DELETE /calendars/{calendarId}/events/{eventId}`" + ` removes one event.
All responses use Google's error envelope for invalid input, missing events,
expired credentials, and unsupported methods.

The fixture has separate deterministic records for two synthetic accounts.
Account A has today's planning and launch-review events, an all-day leave item,
and a recurring review. Account B's event is never visible through account A.
`
