package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"time"

	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
)

type apiRequest struct {
	Query         string         `json:"query"`
	Variables     map[string]any `json:"variables,omitempty"`
	OperationName string         `json:"operationName,omitempty"`
}

type apiResponse struct {
	Data       json.RawMessage `json:"data,omitempty"`
	Errors     json.RawMessage `json:"errors,omitempty"`
	Extensions json.RawMessage `json:"extensions,omitempty"`
}

func (r *apiResponse) err() error {
	if len(r.Errors) > 0 && string(r.Errors) != "null" && string(r.Errors) != "[]" {
		return errors.New("GraphQL returned errors; inspect the JSON response")
	}
	return nil
}

type localClient struct {
	http    *http.Client
	timeout time.Duration
	output  io.Writer
}

func newLocalClient(path string, timeout time.Duration, output io.Writer) *localClient {
	transport := &http.Transport{DialContext: func(ctx context.Context, _, _ string) (net.Conn, error) {
		conn, err := (&net.Dialer{Timeout: timeout}).DialContext(ctx, "unix", path)
		if err != nil {
			return nil, fmt.Errorf("connect to %s: %w; check the server, socket setting, and file permissions", path, err)
		}
		return conn, nil
	}}
	return &localClient{http: &http.Client{Transport: transport, CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse }}, timeout: timeout, output: output}
}
func (l *localClient) emit(value any) error { return json.NewEncoder(l.output).Encode(value) }
func (l *localClient) query(ctx context.Context, value apiRequest) (*apiResponse, error) {
	body, err := json.Marshal(value)
	if err != nil {
		return nil, err
	}
	if len(body) > 64*1024 {
		return nil, usageError("request exceeds the local socket limit of 64 KiB")
	}
	ctx, cancel := context.WithTimeout(ctx, l.timeout)
	defer cancel()
	request, err := http.NewRequestWithContext(ctx, http.MethodPost, "http://noema.local/graphql", bytes.NewReader(body))
	if err != nil {
		return nil, err
	}
	request.Header.Set("Content-Type", "application/json")
	response, err := l.http.Do(request)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	var result apiResponse
	if err = json.NewDecoder(io.LimitReader(response.Body, 8*1024*1024)).Decode(&result); err != nil {
		if response.StatusCode != http.StatusOK {
			return nil, fmt.Errorf("Noema returned HTTP %d", response.StatusCode)
		}
		return nil, fmt.Errorf("invalid GraphQL response: %w", err)
	}
	if len(result.Data) == 0 && len(result.Errors) == 0 {
		return nil, errors.New("empty GraphQL response")
	}
	if response.StatusCode != http.StatusOK {
		if graphqlErr := result.err(); graphqlErr != nil {
			return &result, fmt.Errorf("Noema returned HTTP %d: %w", response.StatusCode, graphqlErr)
		}
		return &result, fmt.Errorf("Noema returned HTTP %d", response.StatusCode)
	}
	return &result, result.err()
}
func (l *localClient) printQuery(ctx context.Context, query string, variables map[string]any) error {
	response, err := l.query(ctx, apiRequest{Query: query, Variables: variables})
	if response != nil {
		if e := l.emit(response); e != nil {
			return e
		}
	}
	return err
}
func (l *localClient) schema(ctx context.Context) error {
	ctx, cancel := context.WithTimeout(ctx, l.timeout)
	defer cancel()
	request, _ := http.NewRequestWithContext(ctx, http.MethodGet, "http://noema.local/graphql/schema.graphql", nil)
	response, err := l.http.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	if response.StatusCode != 200 {
		return fmt.Errorf("schema request returned HTTP %d", response.StatusCode)
	}
	_, err = io.Copy(l.output, response.Body)
	return err
}

type socketFrame struct {
	ID      string          `json:"id,omitempty"`
	Type    string          `json:"type"`
	Payload json.RawMessage `json:"payload,omitempty"`
}

// stream owns one subscription. The callback can send a mutation after readiness.
func (l *localClient) stream(ctx context.Context, request apiRequest, consume func(*apiResponse) (bool, error)) error {
	setup, cancel := context.WithTimeout(ctx, l.timeout)
	connection, _, err := websocket.Dial(setup, "ws://noema.local/graphql/ws", &websocket.DialOptions{HTTPClient: l.http, Subprotocols: []string{"graphql-transport-ws"}})
	if err != nil {
		cancel()
		return err
	}
	defer connection.CloseNow()
	connection.SetReadLimit(8 * 1024 * 1024)
	if err = wsjson.Write(setup, connection, socketFrame{Type: "connection_init"}); err != nil {
		cancel()
		return err
	}
	for {
		var frame socketFrame
		if err = wsjson.Read(setup, connection, &frame); err != nil {
			cancel()
			return err
		}
		if frame.Type == "ping" {
			if err = wsjson.Write(setup, connection, socketFrame{Type: "pong", Payload: frame.Payload}); err != nil {
				cancel()
				return err
			}
			continue
		}
		if frame.Type != "connection_ack" {
			cancel()
			return errors.New("invalid GraphQL connection acknowledgement")
		}
		break
	}
	payload, err := json.Marshal(request)
	if err != nil {
		cancel()
		return err
	}
	err = wsjson.Write(setup, connection, socketFrame{ID: "1", Type: "subscribe", Payload: payload})
	cancel()
	if err != nil {
		return err
	}
	defer func() {
		end, stop := context.WithTimeout(context.Background(), time.Second)
		defer stop()
		_ = wsjson.Write(end, connection, socketFrame{ID: "1", Type: "complete"})
	}()
	readContext := ctx
	var readyCancel context.CancelFunc
	if consume != nil {
		readContext, readyCancel = context.WithTimeout(ctx, l.timeout)
		defer readyCancel()
	}
	for {
		var frame socketFrame
		if err = wsjson.Read(readContext, connection, &frame); err != nil {
			return fmt.Errorf("subscription ended: %w; read current state before retrying a write", err)
		}
		switch frame.Type {
		case "ping":
			if err = wsjson.Write(ctx, connection, socketFrame{Type: "pong", Payload: frame.Payload}); err != nil {
				return err
			}
		case "pong":
		case "complete":
			if consume != nil {
				return errors.New("subscription completed before the Chat turn finished")
			}
			return nil
		case "error":
			if err = l.emit(&apiResponse{Errors: frame.Payload}); err != nil {
				return err
			}
			return errors.New("GraphQL subscription failed")
		case "next":
			var response apiResponse
			if err = json.Unmarshal(frame.Payload, &response); err != nil {
				return err
			}
			if err = response.err(); err != nil {
				if e := l.emit(&response); e != nil {
					return e
				}
				return err
			}
			if consume != nil {
				done, e := consume(&response)
				readContext = ctx
				if readyCancel != nil {
					readyCancel()
				}
				if e != nil {
					return e
				}
				if done {
					return nil
				}
			} else if err = l.emit(&response); err != nil {
				return err
			}
		default:
			return fmt.Errorf("unexpected subscription frame %q", frame.Type)
		}
	}
}
