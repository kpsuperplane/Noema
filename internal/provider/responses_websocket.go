package provider

import (
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"strings"
	"time"

	"github.com/coder/websocket"
)

type responsesWebSocketErrorKind uint8

const (
	responsesWebSocketSetup responsesWebSocketErrorKind = iota
	responsesWebSocketUnsupported
	responsesWebSocketAuthentication
	responsesWebSocketMissingPrevious
	responsesWebSocketFatal
)

type responsesWebSocketError struct {
	kind responsesWebSocketErrorKind
	err  error
}

func (e *responsesWebSocketError) Error() string { return e.err.Error() }
func (e *responsesWebSocketError) Unwrap() error { return e.err }

type responsesWebSocketSession struct {
	url, previousResponseID string
	client                  *http.Client
	timeout                 time.Duration
	connection              *websocket.Conn
	unsupported             bool
	hasHostedWebState       bool
}

func newResponsesWebSocketSession(client *http.Client, responsesURL string) *responsesWebSocketSession {
	url := strings.Replace(strings.Replace(responsesURL, "https://", "wss://", 1), "http://", "ws://", 1)
	webSocketClient := *client
	webSocketClient.Timeout = 0
	return &responsesWebSocketSession{url: url, client: &webSocketClient, timeout: client.Timeout}
}

func (s *responsesWebSocketSession) Close() error {
	if s.connection == nil {
		return nil
	}
	err := s.connection.CloseNow()
	s.connection = nil
	return err
}

func (s *responsesWebSocketSession) closeNow() {
	if s.connection != nil {
		_ = s.connection.CloseNow()
		s.connection = nil
	}
}

func (s *responsesWebSocketSession) send(
	ctx context.Context,
	body []byte,
	headers http.Header,
	token string,
	onEvent func(StreamEvent),
) (codexStreamResult, error) {
	if onEvent == nil {
		onEvent = func(StreamEvent) {}
	}
	requestContext := ctx
	var cancel context.CancelFunc
	if s.timeout > 0 {
		requestContext, cancel = context.WithTimeout(ctx, s.timeout)
		defer cancel()
	}
	if s.unsupported {
		return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketUnsupported, ErrProviderUnavailable}
	}
	if s.connection == nil {
		if err := s.connect(requestContext, headers, token); err != nil {
			return codexStreamResult{}, err
		}
	}
	var message map[string]json.RawMessage
	if decodeUniqueJSON(body, &message) != nil {
		return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketFatal, ErrProviderRequestRejected}
	}
	message["type"] = json.RawMessage(`"response.create"`)
	encoded, err := json.Marshal(message)
	if err != nil {
		return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketFatal, ErrProviderRequestRejected}
	}
	if err := s.connection.Write(requestContext, websocket.MessageText, encoded); err != nil {
		return codexStreamResult{}, s.ioError(ctx, requestContext, false, err)
	}

	result := codexStreamResult{}
	output := make(map[int]json.RawMessage)
	text := make(map[int]string)
	hostedStarted := make(map[int]struct{})
	remaining := int64(codexGenerationResponseLimit)
	sawOutput := false
	for {
		kind, data, err := s.connection.Read(requestContext)
		if err != nil {
			return codexStreamResult{}, s.ioError(ctx, requestContext, sawOutput, err)
		}
		if kind != websocket.MessageText && kind != websocket.MessageBinary {
			continue
		}
		remaining -= int64(len(data))
		if remaining < 0 {
			s.closeNow()
			return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketFatal, errCodexGenerationTooLarge}
		}
		var event map[string]json.RawMessage
		if decodeUniqueJSON(data, &event) != nil {
			s.closeNow()
			return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketSetup, ErrProviderUnavailable}
		}
		eventType, _ := rawString(event["type"])
		sawOutput = sawOutput || responsesEventHasOutput(eventType)
		if responsesErrorCode(event) == "previous_response_not_found" {
			if sawOutput {
				s.closeNow()
				return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketFatal, ErrProviderUnavailable}
			}
			return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketMissingPrevious, ErrProviderRequestRejected}
		}
		if err := consumeCodexGenerationEvent("", data, &result, output, text, hostedStarted, onEvent); err != nil {
			s.closeNow()
			return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketFatal, err}
		}
		if eventType == "response.completed" || eventType == "response.incomplete" {
			if err := reconcileCodexStreamText(output, text); err != nil {
				s.closeNow()
				return codexStreamResult{}, &responsesWebSocketError{responsesWebSocketFatal, err}
			}
			for index := 0; index < maxItems; index++ {
				if item, exists := output[index]; exists {
					result.Output = append(result.Output, codexOutputItem{Index: index, Raw: item})
				}
			}
			return result, nil
		}
	}
}

func (s *responsesWebSocketSession) connect(ctx context.Context, headers http.Header, token string) error {
	headers = headers.Clone()
	if headers == nil {
		headers = make(http.Header)
	}
	headers.Set("Authorization", "Bearer "+token)
	connection, response, err := websocket.Dial(ctx, s.url, &websocket.DialOptions{
		HTTPClient: s.client, HTTPHeader: headers, CompressionMode: websocket.CompressionDisabled,
	})
	headers.Del("Authorization")
	if response != nil && response.Request != nil {
		response.Request.Header.Del("Authorization")
	}
	if err == nil {
		connection.SetReadLimit(codexGenerationResponseLimit)
		s.connection = connection
		return nil
	}
	if ctx.Err() != nil {
		return &responsesWebSocketError{responsesWebSocketFatal, ctx.Err()}
	}
	if response == nil {
		return &responsesWebSocketError{responsesWebSocketSetup, ErrProviderUnavailable}
	}
	if response.Body != nil {
		_ = response.Body.Close()
	}
	switch response.StatusCode {
	case http.StatusNotFound, http.StatusMethodNotAllowed, http.StatusUpgradeRequired, http.StatusNotImplemented:
		s.unsupported = true
		return &responsesWebSocketError{responsesWebSocketUnsupported, ErrProviderUnavailable}
	case http.StatusUnauthorized, http.StatusForbidden:
		return &responsesWebSocketError{responsesWebSocketAuthentication, ErrAuthenticationRejected}
	case http.StatusTooManyRequests:
		return &responsesWebSocketError{responsesWebSocketFatal, ErrProviderRateLimited}
	default:
		return &responsesWebSocketError{responsesWebSocketFatal, ErrProviderUnavailable}
	}
}

func (s *responsesWebSocketSession) ioError(parent, request context.Context, sawOutput bool, _ error) error {
	s.closeNow()
	if parent.Err() != nil {
		return &responsesWebSocketError{responsesWebSocketFatal, parent.Err()}
	}
	if request.Err() != nil {
		return &responsesWebSocketError{responsesWebSocketFatal, ErrProviderUnavailable}
	}
	if sawOutput {
		return &responsesWebSocketError{responsesWebSocketFatal, ErrProviderUnavailable}
	}
	return &responsesWebSocketError{responsesWebSocketSetup, ErrProviderUnavailable}
}

func responsesEventHasOutput(eventType string) bool {
	switch eventType {
	case "response.output_text.delta", "response.output_item.added", "response.output_item.done",
		"response.reasoning_summary_text.delta", "response.reasoning_text.delta", "response.reasoning_summary_text.done", "response.reasoning_text.done",
		"response.web_search_call.in_progress", "response.web_search_call.searching",
		"response.web_search_call.completed":
		return true
	default:
		return false
	}
}

func responsesErrorCode(event map[string]json.RawMessage) string {
	for _, raw := range []json.RawMessage{event["error"], event["response"]} {
		var value map[string]json.RawMessage
		if json.Unmarshal(raw, &value) != nil {
			continue
		}
		if nested := value["error"]; len(nested) != 0 {
			_ = json.Unmarshal(nested, &value)
		}
		code, _ := rawString(value["code"])
		if code != "" {
			return code
		}
	}
	return ""
}

func responsesWebSocketKind(err error) responsesWebSocketErrorKind {
	var websocketError *responsesWebSocketError
	if errors.As(err, &websocketError) {
		return websocketError.kind
	}
	return responsesWebSocketFatal
}
