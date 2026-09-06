package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"slices"
	"strings"

	"github.com/google/uuid"
	"github.com/urfave/cli/v3"
)

const taskSummaryFields = `taskId title revision generation stage{stageId name behavior} validActions createdAt updatedAt`
const taskDetailFields = taskSummaryFields + ` taskDocument taskDocumentDigest resultDocument reviewDocument schedule{scheduledFor timeZone}`
const taskReadQuery = `query($id:String!){task(taskId:$id){` + taskDetailFields + `}}`
const taskListQuery = `query($input:TaskListInput!,$first:Int,$after:String){tasks(input:$input,first:$first,after:$after){edges{cursor node{` + taskSummaryFields + `}}pageInfo{endCursor hasNextPage}}}`
const taskWatchQuery = `subscription($id:String!,$after:String){taskEvents(taskId:$id,after:$after){cursor eventId kind occurredAt taskId runId actor payload}}`

func requestID(c *cli.Command) string {
	if value := c.String("request-id"); value != "" {
		return value
	}
	return uuid.NewString()
}
func taskMutationQuery(name, input string) string {
	return `mutation($input:` + input + `!){` + name + `(input:$input){task{` + taskDetailFields + `}eventCursor clientMutationId}}`
}

type clientAction func(func(context.Context, *cli.Command, *localClient) error) cli.ActionFunc

func taskCommands(input io.Reader, action clientAction) *cli.Command {
	return &cli.Command{Name: "tasks", Usage: "Create, inspect, and run Tasks", Commands: []*cli.Command{
		{Name: "list", Usage: "List one page of Tasks", Flags: []cli.Flag{
			&cli.StringFlag{Name: "workspace", Value: "workspace:personal"}, stringFlag("project", "Filter by project ID"),
			&cli.StringFlag{Name: "scope", Value: "ACTIVE", Usage: "ACTIVE, TERMINAL, or ALL"}, &cli.StringSliceFlag{Name: "status", Usage: "Filter by workflow stage behavior"},
			&cli.IntFlag{Name: "first", Value: 50}, stringFlag("after", "Continue after this cursor"),
		}, Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			scope := strings.ToUpper(c.String("scope"))
			if !slices.Contains([]string{"ACTIVE", "TERMINAL", "ALL"}, scope) {
				return usageError("scope must be ACTIVE, TERMINAL, or ALL")
			}
			values := map[string]any{"workspaceId": c.String("workspace"), "scope": scope}
			if c.IsSet("project") {
				values["projectId"] = c.String("project")
			}
			if c.IsSet("status") {
				values["stageBehaviors"] = c.StringSlice("status")
			}
			variables := map[string]any{"input": values, "first": c.Int("first")}
			if c.IsSet("after") {
				variables["after"] = c.String("after")
			}
			return l.printQuery(ctx, taskListQuery, variables)
		})},
		{Name: "get", Usage: "Read Task details", ArgsUsage: "ID", Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 1); err != nil {
				return err
			}
			return l.printQuery(ctx, taskReadQuery, map[string]any{"id": c.Args().First()})
		})},
		{Name: "create", Usage: "Capture a Task in Inbox", ArgsUsage: "TITLE", Flags: []cli.Flag{
			&cli.StringFlag{Name: "workspace", Value: "workspace:personal"}, stringFlag("project", "Project ID"), stringFlag("file", "Task document file; - reads stdin"), stringFlag("request-id", "Explicit mutation request ID"),
		}, Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 1); err != nil {
				return err
			}
			values := map[string]any{"workspaceId": c.String("workspace"), "title": c.Args().First(), "clientMutationId": requestID(c)}
			if c.IsSet("project") {
				values["projectId"] = c.String("project")
			}
			if c.IsSet("file") {
				body, err := readCommandFile(c.String("file"), input)
				if err != nil {
					return err
				}
				values["taskDocument"] = string(body)
			}
			return l.printQuery(ctx, taskMutationQuery("captureTask", "CaptureTaskInput"), map[string]any{"input": values})
		})},
		{Name: "run", Usage: "Queue a Task or start a scheduled Task now", ArgsUsage: "ID", Flags: []cli.Flag{stringFlag("request-id", "Explicit mutation request ID")}, Action: action(changeTask)},
		{Name: "cancel", Usage: "Cancel a nonterminal Task", ArgsUsage: "ID", Flags: []cli.Flag{stringFlag("request-id", "Explicit mutation request ID"), stringFlag("reason", "Cancellation reason")}, Action: action(changeTask)},
		{Name: "watch", Usage: "Stream Task events", ArgsUsage: "ID", Flags: []cli.Flag{stringFlag("after", "Resume after this event cursor")}, Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 1); err != nil {
				return err
			}
			variables := map[string]any{"id": c.Args().First()}
			if c.IsSet("after") {
				variables["after"] = c.String("after")
			}
			return l.stream(ctx, apiRequest{Query: taskWatchQuery, Variables: variables}, nil)
		})},
	}}
}

func changeTask(ctx context.Context, c *cli.Command, l *localClient) error {
	if err := argumentCount(c, 1); err != nil {
		return err
	}
	response, err := l.query(ctx, apiRequest{Query: taskReadQuery, Variables: map[string]any{"id": c.Args().First()}})
	if err != nil {
		if response != nil {
			if e := l.emit(response); e != nil {
				return e
			}
		}
		return err
	}
	var data struct {
		Task struct {
			Revision     int
			Generation   int
			ValidActions []string
		}
	}
	if err = json.Unmarshal(response.Data, &data); err != nil {
		return err
	}
	name, input, required := "cancelTask", "CancelTaskInput", "CANCEL"
	if c.Name == "run" {
		name, input, required = "queueTask", "QueueTaskInput", "QUEUE"
		if slices.Contains(data.Task.ValidActions, "RUN_NOW") {
			name, input, required = "runScheduledTaskNow", "RunScheduledTaskNowInput", "RUN_NOW"
		}
	}
	if !slices.Contains(data.Task.ValidActions, required) {
		return errors.New("Task does not currently permit this action")
	}
	values := map[string]any{"taskId": c.Args().First(), "expectedRevision": data.Task.Revision, "expectedGeneration": data.Task.Generation, "clientMutationId": requestID(c)}
	if c.Name == "cancel" && c.IsSet("reason") {
		values["reason"] = c.String("reason")
	}
	return l.printQuery(ctx, taskMutationQuery(name, input), map[string]any{"input": values})
}
