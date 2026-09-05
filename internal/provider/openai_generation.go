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
	body, toolNames, err := prepareResponsesGeneration(request, responsesGenerationProfile{
		accountID: openAIDefaultAccountID, providerName: "OpenAI",
		promptCacheRetention: "24h", forwardMaxOutput: true,
		includeEncryptedReasoning: true, store: true, stream: true,
	})
	if err != nil {
		return GenerationResult{}, err
	}
	account, err := g.accounts.LoadAccount(ctx, request.AccountID)
	if err != nil {
		return GenerationResult{}, err
	}
	if account.ID != openAIDefaultAccountID || account.ProviderKind != "openai" ||
		!account.IsActive || account.Status != StatusAuthenticated {
		return GenerationResult{}, ErrAuthenticationRejected
	}
	secret, err := g.accounts.LoadSecret(ctx, request.AccountID)
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
