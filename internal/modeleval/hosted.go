package modeleval

import (
	"errors"
	"strings"
)

const openRouterDefaultAccountID = "provider_account:openrouter:default"

type openRouterEnvCredentials struct {
	apiKey string
}

func (c openRouterEnvCredentials) apiKeyFor(providerKind, providerAccountID string) (string, error) {
	if providerKind == "openrouter" && providerAccountID == openRouterDefaultAccountID {
		return c.apiKey, nil
	}
	return "", errors.New("missing OpenRouter API key")
}

func openrouterAPIKey(value *string) (string, error) {
	if value == nil {
		return "", errors.New("OPENROUTER_API_KEY is not set or is blank")
	}
	trimmed := strings.TrimSpace(*value)
	if trimmed == "" {
		return "", errors.New("OPENROUTER_API_KEY is not set or is blank")
	}
	return trimmed, nil
}
