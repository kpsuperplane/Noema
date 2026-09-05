package runtime

import (
	"encoding/json"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/webtool"
)

type evaluationStep struct {
	tool    string
	exact   map[string]string
	topical map[string][]string
	output  json.RawMessage
}
type evaluationScenario struct {
	id, user, final string
	steps           []evaluationStep
}

func evaluationScenarios() []evaluationScenario {
	return []evaluationScenario{
		{id: "primary_stateful_flight_to_calendar", user: "Can you add AS385 on Sep 17 to my calendar?", final: "final", steps: []evaluationStep{
			{tool: "web.search", exact: map[string]string{}, topical: map[string][]string{"query": {"as385", "as 385", "alaska 385"}}, output: json.RawMessage("{\"query\":\"evaluation query\",\"provider\":\"evaluation\",\"results\":[{\"rank\":1,\"title\":\"AS385 flight status and schedule\",\"url\":\"https://fixtures.noema.test/flights/as385/2026-09-17\",\"snippet\":\"Official itinerary details for AS385 from Seattle to Toronto on September 17, 2026.\"}]}")},
			{tool: "web.fetch", exact: map[string]string{"url": "https://fixtures.noema.test/flights/as385/2026-09-17"}, topical: map[string][]string{}, output: json.RawMessage("{\"url\":\"https://fixtures.noema.test/flights/as385/2026-09-17\",\"title\":\"AS385 itinerary\",\"content\":\"AS385 departs Seattle (SEA) September 17, 2026 at 7:52 AM PDT and arrives Toronto (YYZ) at 3:45 PM EDT. Scheduled timestamps: 2026-09-17T07:52:00-07:00 to 2026-09-17T15:45:00-04:00.\"}")},
			{tool: "calendar.create_event", exact: map[string]string{"calendarId": "primary", "start_dateTime": "2026-09-17T07:52:00-07:00", "end_dateTime": "2026-09-17T15:45:00-04:00"}, topical: map[string][]string{"summary": {"as385", "as 385"}}, output: json.RawMessage("{\"created\":true,\"event_id\":\"evt-as385\",\"calendar_id\":\"primary\"}")},
		}},
		{id: "primary_stateful_public_event_to_calendar", user: "Put the Northstar Data Summit opening keynote on my calendar.", final: "final", steps: []evaluationStep{
			{tool: "web.search", exact: map[string]string{}, topical: map[string][]string{"query": {"northstar", "data summit", "keynote"}}, output: json.RawMessage("{\"query\":\"evaluation query\",\"provider\":\"evaluation\",\"results\":[{\"rank\":1,\"title\":\"Northstar Data Summit 2026 agenda\",\"url\":\"https://fixtures.noema.test/northstar-2026/agenda\",\"snippet\":\"Official conference agenda including the opening keynote.\"}]}")},
			{tool: "web.fetch", exact: map[string]string{"url": "https://fixtures.noema.test/northstar-2026/agenda"}, topical: map[string][]string{}, output: json.RawMessage("{\"url\":\"https://fixtures.noema.test/northstar-2026/agenda\",\"title\":\"Northstar Data Summit 2026 agenda\",\"content\":\"Opening keynote: Reliable Systems at Human Scale. October 6, 2026, 9:00–10:15 AM America/Los_Angeles. Venue: Summit Hall. Timestamps: 2026-10-06T09:00:00-07:00 to 2026-10-06T10:15:00-07:00.\"}")},
			{tool: "calendar.create_event", exact: map[string]string{"calendarId": "primary", "start_dateTime": "2026-10-06T09:00:00-07:00", "end_dateTime": "2026-10-06T10:15:00-07:00"}, topical: map[string][]string{"summary": {"northstar", "keynote", "reliable systems"}}, output: json.RawMessage("{\"created\":true,\"event_id\":\"evt-northstar-keynote\",\"calendar_id\":\"primary\"}")},
		}},
		{id: "primary_stateful_email_meeting_to_calendar", user: "Put my Rowan Labs interview on my calendar.", final: "final", steps: []evaluationStep{
			{tool: "gmail.list_messages", exact: map[string]string{}, topical: map[string][]string{"query": {"rowan", "interview", "recruit"}}, output: json.RawMessage("{\"messages\":[{\"id\":\"msg-rowan-interview\",\"thread_id\":\"thread-rowan\",\"from\":\"Maya Chen <maya@rowan.example>\",\"subject\":\"Rowan Labs interview details\",\"received_at\":\"2026-07-14T16:20:00-07:00\",\"snippet\":\"Here are the details for your interview...\"}]}")},
			{tool: "gmail.get_message", exact: map[string]string{"message_id": "msg-rowan-interview"}, topical: map[string][]string{}, output: json.RawMessage("{\"id\":\"msg-rowan-interview\",\"from\":\"Maya Chen <maya@rowan.example>\",\"subject\":\"Rowan Labs interview details\",\"body\":\"Your interview with Rowan Labs is confirmed for July 22, 2026 from 11:30 AM to 12:15 PM Pacific. Video call: https://meet.example/rowan. Timestamps: 2026-07-22T11:30:00-07:00 to 2026-07-22T12:15:00-07:00.\"}")},
			{tool: "calendar.create_event", exact: map[string]string{"calendarId": "primary", "start_dateTime": "2026-07-22T11:30:00-07:00", "end_dateTime": "2026-07-22T12:15:00-07:00"}, topical: map[string][]string{"summary": {"rowan", "interview"}}, output: json.RawMessage("{\"created\":true,\"event_id\":\"evt-rowan-interview\",\"calendar_id\":\"primary\"}")},
		}},
		{id: "primary_stateful_email_reschedule", user: "Make sure my calendar has the latest time for my Rowan Labs interview.", final: "final", steps: []evaluationStep{
			{tool: "gmail.list_messages", exact: map[string]string{}, topical: map[string][]string{"query": {"rowan", "interview", "recruit"}}, output: json.RawMessage("{\"messages\":[{\"id\":\"msg-rowan-reschedule\",\"thread_id\":\"thread-rowan\",\"from\":\"Maya Chen <maya@rowan.example>\",\"subject\":\"Updated Rowan Labs interview time\",\"received_at\":\"2026-07-15T08:30:00-07:00\",\"snippet\":\"We need to move your interview...\"},{\"id\":\"msg-rowan-interview\",\"thread_id\":\"thread-rowan\",\"from\":\"Maya Chen <maya@rowan.example>\",\"subject\":\"Rowan Labs interview details\",\"received_at\":\"2026-07-14T16:20:00-07:00\",\"snippet\":\"Your interview is confirmed...\"}]}")},
			{tool: "gmail.get_message", exact: map[string]string{"message_id": "msg-rowan-reschedule"}, topical: map[string][]string{}, output: json.RawMessage("{\"id\":\"msg-rowan-reschedule\",\"from\":\"Maya Chen <maya@rowan.example>\",\"subject\":\"Updated Rowan Labs interview time\",\"body\":\"Your Rowan Labs interview moved to July 22, 2026 from 1:00 PM to 1:45 PM Pacific. This replaces the earlier 11:30 AM time. Timestamps: 2026-07-22T13:00:00-07:00 to 2026-07-22T13:45:00-07:00.\"}")},
			{tool: "calendar.list_events", exact: map[string]string{"calendarId": "primary"}, topical: map[string][]string{"query": {"rowan", "interview"}}, output: json.RawMessage("{\"events\":[{\"id\":\"evt-rowan-existing\",\"summary\":\"Rowan Labs interview\",\"start\":\"2026-07-22T11:30:00-07:00\",\"end\":\"2026-07-22T12:15:00-07:00\"}]}")},
			{tool: "calendar.update_event", exact: map[string]string{"calendarId": "primary", "eventId": "evt-rowan-existing", "start_dateTime": "2026-07-22T13:00:00-07:00", "end_dateTime": "2026-07-22T13:45:00-07:00"}, topical: map[string][]string{"summary": {"rowan", "interview"}}, output: json.RawMessage("{\"updated\":true,\"event_id\":\"evt-rowan-existing\",\"calendar_id\":\"primary\"}")},
		}},
		{id: "primary_stateful_package_delivery", user: "When are my new headphones getting here?", final: "delivery", steps: []evaluationStep{
			{tool: "gmail.list_messages", exact: map[string]string{}, topical: map[string][]string{"query": {"headphones", "shipping", "delivery", "order"}}, output: json.RawMessage("{\"messages\":[{\"id\":\"msg-headphones-shipped\",\"thread_id\":\"thread-headphones-order\",\"from\":\"Northstar Audio <shipping@northstaraudio.example>\",\"subject\":\"Your headphones have shipped\",\"received_at\":\"2026-07-15T07:10:00-07:00\",\"snippet\":\"Your delivery is on the way...\"}]}")},
			{tool: "gmail.get_message", exact: map[string]string{"message_id": "msg-headphones-shipped"}, topical: map[string][]string{}, output: json.RawMessage("{\"id\":\"msg-headphones-shipped\",\"subject\":\"Your headphones have shipped\",\"body\":\"Your Northstar Arc headphones are scheduled for delivery on August 12, 2026 by 8:00 PM. Tracking number: NS-481516.\"}")},
		}},
		{id: "primary_stateful_passport_reminder", user: "Make sure I don't miss the passport renewal deadline from that email.", final: "final", steps: []evaluationStep{
			{tool: "gmail.list_messages", exact: map[string]string{}, topical: map[string][]string{"query": {"passport", "renew", "expiration"}}, output: json.RawMessage("{\"messages\":[{\"id\":\"msg-passport-renewal\",\"thread_id\":\"thread-passport-renewal\",\"from\":\"Travel Documents <notices@travel.example>\",\"subject\":\"Passport renewal window\",\"received_at\":\"2026-07-12T09:00:00-07:00\",\"snippet\":\"Renew by November 1 to allow processing time...\"}]}")},
			{tool: "gmail.get_message", exact: map[string]string{"message_id": "msg-passport-renewal"}, topical: map[string][]string{}, output: json.RawMessage("{\"id\":\"msg-passport-renewal\",\"subject\":\"Passport renewal window\",\"body\":\"Your passport expires February 1, 2027. Submit the renewal by November 1, 2026 to allow processing time. Reminder timestamp: 2026-11-01T09:00:00-07:00.\"}")},
			{tool: "reminders.create_reminder", exact: map[string]string{"due_dateTime": "2026-11-01T09:00:00-07:00"}, topical: map[string][]string{"title": {"passport", "renew"}}, output: json.RawMessage("{\"created\":true,\"reminder_id\":\"reminder-passport-renewal\"}")},
		}},
		{id: "primary_stateful_missing_appointment", user: "Put the dentist appointment from my latest email on my calendar.", final: "final", steps: []evaluationStep{
			{tool: "gmail.list_messages", exact: map[string]string{}, topical: map[string][]string{"query": {"dentist", "appointment", "dental"}}, output: json.RawMessage("{\"messages\":[],\"exhaustive\":false,\"suggested_query\":\"in:anywhere\"}")},
			{tool: "gmail.list_messages", exact: map[string]string{}, topical: map[string][]string{"query": {}}, output: json.RawMessage("{\"messages\":[],\"exhaustive\":true}")},
		}},
	}
}

func evaluationActionTools() []provider.GenerationTool {
	tools := []provider.GenerationTool{
		{Name: webtool.SearchName, Description: "Search the public web.", InputSchema: webtool.SearchSchema},
		{Name: webtool.FetchName, Description: "Read the exact discovered web page URL.", InputSchema: webtool.FetchSchema},
		{Name: "calendar.create_event", Description: "Create an external calendar event with exact RFC3339 start and end times. Use calendarId primary for the user's default calendar.", InputSchema: json.RawMessage("{\"type\":\"object\",\"properties\":{\"calendarId\":{\"type\":\"string\",\"description\":\"Use primary for the user's default calendar.\"},\"start_dateTime\":{\"type\":\"string\"},\"end_dateTime\":{\"type\":\"string\"},\"summary\":{\"type\":\"string\"}},\"required\":[\"calendarId\",\"start_dateTime\",\"end_dateTime\",\"summary\"],\"additionalProperties\":false}")},
		{Name: "calendar.list_events", Description: "List matching events from an external calendar before updating an existing event. Use calendarId primary for the user's default calendar.", InputSchema: json.RawMessage("{\"type\":\"object\",\"properties\":{\"calendarId\":{\"type\":\"string\"},\"query\":{\"type\":\"string\"},\"timeMin\":{\"type\":\"string\"},\"timeMax\":{\"type\":\"string\"}},\"required\":[\"calendarId\",\"query\"],\"additionalProperties\":false}")},
		{Name: "calendar.update_event", Description: "Update one external calendar event using the exact event identifier returned by calendar.list_events.", InputSchema: json.RawMessage("{\"type\":\"object\",\"properties\":{\"calendarId\":{\"type\":\"string\"},\"eventId\":{\"type\":\"string\"},\"start_dateTime\":{\"type\":\"string\"},\"end_dateTime\":{\"type\":\"string\"},\"summary\":{\"type\":\"string\"}},\"required\":[\"calendarId\",\"eventId\",\"start_dateTime\",\"end_dateTime\",\"summary\"],\"additionalProperties\":false}")},
		{Name: "reminders.create_reminder", Description: "Create a reminder at an exact RFC3339 time grounded in an authoritative source.", InputSchema: json.RawMessage("{\"type\":\"object\",\"properties\":{\"title\":{\"type\":\"string\"},\"due_dateTime\":{\"type\":\"string\"}},\"required\":[\"title\",\"due_dateTime\"],\"additionalProperties\":false}")},
		{Name: "gmail.list_messages", Description: "Search the connected Gmail mailbox and return matching message summaries and identifiers.", InputSchema: json.RawMessage("{\"type\":\"object\",\"properties\":{\"query\":{\"type\":\"string\"},\"max_results\":{\"type\":\"integer\"}},\"required\":[\"query\"],\"additionalProperties\":false}")},
		{Name: "gmail.get_message", Description: "Read one Gmail message by the exact identifier returned by gmail.list_messages.", InputSchema: json.RawMessage("{\"type\":\"object\",\"properties\":{\"message_id\":{\"type\":\"string\"}},\"required\":[\"message_id\"],\"additionalProperties\":false}")},
	}
	return append(tools, taskToolSpecs...)
}
