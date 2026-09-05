package provider

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"net/http"
	"strings"
	"time"
)

const (
	openAIResponsesURL      = "https://api.openai.com/v1/responses"
	openAIGenerationTimeout = 120 * time.Second
)

var errOpenAIGenerationTooLarge = errors.New("OpenAI generation response is too large")

// OpenAIGenerator sends OpenAI Responses requests.
type OpenAIGenerator struct {
	accounts     *AccountService
	client       *http.Client
	responsesURL string
}

type openAIGenerationSession struct {
	provider  *OpenAIGenerator
	responses *responsesWebSocketSession
}

// NewOpenAIGenerator creates the production OpenAI generation transport.
func NewOpenAIGenerator(accounts *AccountService) (*OpenAIGenerator, error) {
	return newOpenAIGenerator(
		accounts, &http.Client{Timeout: openAIGenerationTimeout}, openAIResponsesURL,
	)
}

func newOpenAIGenerator(
	accounts *AccountService,
	client *http.Client,
	responsesURL string,
) (*OpenAIGenerator, error) {
	if accounts == nil || client == nil {
		return nil, errors.New("OpenAI generation dependencies are unavailable")
	}
	parsed, err := exactHTTPURL(responsesURL)
	if err != nil || parsed.RawQuery != "" || parsed.Fragment != "" || parsed.User != nil {
		return nil, errors.New("OpenAI generation URL is invalid")
	}
	boundedClient := *client
	boundedClient.CheckRedirect = func(*http.Request, []*http.Request) error {
		return http.ErrUseLastResponse
	}
	return &OpenAIGenerator{
		accounts: accounts, client: &boundedClient, responsesURL: parsed.String(),
	}, nil
}

// Generate streams text deltas and returns one completed OpenAI response.
func (g *OpenAIGenerator) Generate(
	ctx context.Context,
	request GenerateRequest,
	onEvent func(StreamEvent),
) (GenerationResult, error) {
	body, toolNames, err := prepareResponsesGeneration(request, openAIResponsesProfile())
	if err != nil {
		return GenerationResult{}, err
	}
	account, secret, err := g.accountSecret(ctx, request)
	if err != nil {
		return GenerationResult{}, err
	}
	httpRequest, err := http.NewRequestWithContext(
		ctx, http.MethodPost, g.responsesURL, bytes.NewReader(body),
	)
	if err != nil {
		return GenerationResult{}, errors.New("OpenAI generation request is invalid")
	}
	httpRequest.Header.Set("Content-Type", "application/json")
	httpRequest.Header.Set("Accept", "text/event-stream")
	if err := setOpenAIAccountHeaders(httpRequest, account.Metadata); err != nil {
		return GenerationResult{}, err
	}

	var response *http.Response
	err = secret.Use(func(value string) error {
		httpRequest.Header.Set("Authorization", "Bearer "+value)
		var sendErr error
		response, sendErr = g.client.Do(httpRequest)
		httpRequest.Header.Del("Authorization")
		if response != nil && response.Request != nil {
			response.Request.Header.Del("Authorization")
		}
		return sendErr
	})
	if err != nil {
		if ctx.Err() != nil {
			return GenerationResult{}, ctx.Err()
		}
		return GenerationResult{}, ErrProviderUnavailable
	}
	if response == nil {
		return GenerationResult{}, ErrProviderUnavailable
	}
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		_ = response.Body.Close()
		return GenerationResult{}, codexGenerationStatusError(response.StatusCode)
	}
	parsed, err := parseCodexGenerationStream(ctx, newCodexGenerationStream(response.Body), onEvent)
	if err != nil {
		if ctx.Err() != nil {
			return GenerationResult{}, ctx.Err()
		}
		if errors.Is(err, errCodexGenerationTooLarge) {
			return GenerationResult{}, errOpenAIGenerationTooLarge
		}
		return GenerationResult{}, errors.New("OpenAI returned an invalid generation stream")
	}
	result, err := normalizeCodexGeneration(request, parsed, toolNames)
	if err != nil {
		return GenerationResult{}, errors.New("OpenAI returned an invalid generation response")
	}
	return result, nil
}

// OpenGenerationSession opens one lazy Responses WebSocket session.
func (g *OpenAIGenerator) OpenGenerationSession() GenerationSession {
	return &openAIGenerationSession{provider: g, responses: newResponsesWebSocketSession(g.client, g.responsesURL)}
}

func (s *openAIGenerationSession) Close() error { return s.responses.Close() }

func (s *openAIGenerationSession) ContinuationReady(previousID string) bool {
	return previousID != "" && s.responses.previousResponseID != ""
}

func (s *openAIGenerationSession) Generate(
	ctx context.Context,
	request GenerateRequest,
	onEvent func(StreamEvent),
) (GenerationResult, error) {
	replay := request.Messages
	if request.ReplayMessages != nil {
		replay = request.ReplayMessages
	}
	continuing := s.responses.previousResponseID != "" && request.PreviousResponseID != ""
	if s.responses.hasHostedWebState && !continuing {
		return GenerationResult{}, ErrProviderUnavailable
	}
	prepared := request
	prepared.StoreResponse = true
	prepared.Messages = replay
	prepared.PreviousResponseID = ""
	if continuing {
		prepared.Messages = request.Messages
		prepared.PreviousResponseID = s.responses.previousResponseID
	}
	body, names, err := prepareResponsesGeneration(prepared, openAIResponsesProfile())
	if err != nil {
		return GenerationResult{}, err
	}
	account, secret, err := s.provider.accountSecret(ctx, prepared)
	if err != nil {
		return GenerationResult{}, err
	}
	headers := make(http.Header)
	headerRequest := &http.Request{Header: headers}
	if err := setOpenAIAccountHeaders(headerRequest, account.Metadata); err != nil {
		return GenerationResult{}, err
	}
	var parsed codexStreamResult
	err = secret.Use(func(token string) error {
		var sendErr error
		parsed, sendErr = s.responses.send(ctx, body, headers, token, onEvent)
		return sendErr
	})
	if responsesWebSocketKind(err) == responsesWebSocketMissingPrevious && continuing {
		if s.responses.hasHostedWebState || request.ReplayMessages == nil {
			return GenerationResult{}, ErrProviderUnavailable
		}
		s.responses.previousResponseID = ""
		prepared.Messages, prepared.PreviousResponseID = replay, ""
		body, names, err = prepareResponsesGeneration(prepared, openAIResponsesProfile())
		if err == nil {
			err = secret.Use(func(token string) error {
				var sendErr error
				parsed, sendErr = s.responses.send(ctx, body, headers, token, onEvent)
				return sendErr
			})
		}
	}
	if err != nil {
		kind := responsesWebSocketKind(err)
		if kind != responsesWebSocketSetup && kind != responsesWebSocketUnsupported {
			return GenerationResult{}, err
		}
		if continuing && request.ReplayMessages == nil {
			return GenerationResult{}, ErrProviderUnavailable
		}
		fallback := prepared
		if !continuing || s.responses.previousResponseID == "" {
			fallback.Messages, fallback.PreviousResponseID = replay, ""
		}
		result, fallbackErr := s.provider.Generate(ctx, fallback, onEvent)
		if fallbackErr == nil {
			s.responses.previousResponseID = result.ID
			s.responses.hasHostedWebState = s.responses.hasHostedWebState || len(result.Searches) != 0
		}
		return result, fallbackErr
	}
	result, err := normalizeCodexGeneration(prepared, parsed, names)
	if err != nil {
		return GenerationResult{}, errors.New("OpenAI returned an invalid generation response")
	}
	s.responses.previousResponseID = result.ID
	s.responses.hasHostedWebState = s.responses.hasHostedWebState || len(result.Searches) != 0
	return result, nil
}

func (g *OpenAIGenerator) accountSecret(ctx context.Context, request GenerateRequest) (Account, Secret, error) {
	account, err := g.accounts.LoadAccount(ctx, request.AccountID)
	if err != nil {
		return Account{}, Secret{}, err
	}
	if account.ID != openAIDefaultAccountID || account.ProviderKind != "openai" ||
		!account.IsActive || account.Status != StatusAuthenticated {
		return Account{}, Secret{}, ErrAuthenticationRejected
	}
	if request.ExpectedCredentialRevision != nil &&
		account.Metadata.CredentialRevision() != *request.ExpectedCredentialRevision {
		return Account{}, Secret{}, ErrAccountConflict
	}
	var secret Secret
	if request.ExpectedCredentialRevision == nil {
		secret, err = g.accounts.LoadSecret(ctx, request.AccountID)
	} else {
		secret, err = g.accounts.LoadSecretAtRevision(ctx, request.AccountID, *request.ExpectedCredentialRevision)
	}
	return account, secret, err
}

func openAIResponsesProfile() responsesGenerationProfile {
	return responsesGenerationProfile{
		accountID: openAIDefaultAccountID, providerName: "OpenAI",
		promptCacheRetention: "24h", forwardMaxOutput: true,
		includeEncryptedReasoning: true, promptCacheOptions: true, stream: true,
	}
}

func setOpenAIAccountHeaders(request *http.Request, metadata AccountMetadata) error {
	for key, header := range map[string]string{
		"organization_id": "OpenAI-Organization", "project_id": "OpenAI-Project",
	} {
		var value string
		if raw := metadata[key]; len(raw) != 0 && json.Unmarshal(raw, &value) != nil {
			return errors.New("OpenAI account metadata is invalid")
		}
		value = strings.TrimSpace(value)
		if value == "" {
			continue
		}
		if !validCodexHeader(value) {
			return errors.New("OpenAI account header is invalid")
		}
		request.Header.Set(header, value)
	}
	return nil
}
