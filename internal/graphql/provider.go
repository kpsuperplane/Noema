package graphql

import (
	"context"
	"errors"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/provider"
)

func (r *Resolver) providerAccounts(ctx context.Context) ([]*model.ProviderAccount, error) {
	if r.ProviderAccounts == nil {
		return nil, errors.New("provider account service is unavailable")
	}
	accounts, err := r.ProviderAccounts.Accounts(ctx)
	if err != nil {
		return nil, err
	}
	result := make([]*model.ProviderAccount, 0, len(accounts))
	for _, account := range accounts {
		result = append(result, providerAccountModel(account))
	}
	return result, nil
}

func providerAccountCatalog() []*model.ProviderAccountCatalogEntry {
	entries := provider.Catalog()
	result := make([]*model.ProviderAccountCatalogEntry, 0, len(entries))
	for _, entry := range entries {
		account := provider.Account{ProviderKind: entry.ProviderKind, Status: provider.StatusAuthenticated}
		methods := make([]model.ProviderAuthMethod, 0, len(entry.SupportedAuthMethods))
		for _, method := range entry.SupportedAuthMethods {
			methods = append(methods, providerAuthMethodModel(method))
		}
		result = append(result, &model.ProviderAccountCatalogEntry{
			ProviderKind: entry.ProviderKind, DisplayName: entry.DisplayName,
			PreferredAuthMethod:  providerAuthMethodModel(entry.PreferredAuthMethod),
			SupportedAuthMethods: methods, Capabilities: providerCapabilitiesModel(account),
		})
	}
	return result
}

func (r *Resolver) createProviderAccount(
	ctx context.Context,
	input model.CreateProviderAccountInput,
) (*model.ProviderAccount, error) {
	if r.ProviderAccounts == nil || input.AuthMethod != model.ProviderAuthMethodSecretInput {
		return nil, provider.ErrAuthMethodMismatch
	}
	var (
		account provider.Account
		err     error
	)
	if input.ProviderKind == "openrouter" {
		secret, secretErr := provider.NewSecret(input.Secret)
		if secretErr != nil {
			return nil, secretErr
		}
		if r.OpenRouter == nil {
			return nil, errors.New("OpenRouter onboarding is unavailable")
		}
		account, err = r.OpenRouter.CreateAPIKeyAccount(ctx, secret)
	} else {
		var displayName *string
		if input.DisplayName != nil {
			displayName = input.DisplayName
		}
		request, requestErr := provider.NewCreateSecretProviderAccountRequest(input.ProviderKind, displayName, input.Secret)
		if requestErr != nil {
			return nil, requestErr
		}
		account, err = r.ProviderAccounts.CreateSecretAccountRequest(ctx, request, time.Now())
	}
	if err != nil {
		return nil, err
	}
	return providerAccountModel(account), nil
}

func (r *Resolver) saveProviderSecret(
	ctx context.Context,
	input model.ProviderSecretInput,
) (*model.ProviderAccount, error) {
	if r.ProviderAccounts == nil {
		return nil, errors.New("provider account service is unavailable")
	}
	request, err := provider.NewSaveProviderAccountSecretRequest(input.ProviderAccountID, input.Secret)
	if err != nil {
		return nil, err
	}
	account, err := r.ProviderAccounts.SaveSecretRequest(ctx, request, time.Now())
	if err != nil {
		return nil, err
	}
	return providerAccountModel(account), nil
}

func (r *Resolver) clearProviderSecret(
	ctx context.Context,
	accountID string,
) (*model.ProviderAccount, error) {
	if r.ProviderAccounts == nil {
		return nil, errors.New("provider account service is unavailable")
	}
	account, err := r.ProviderAccounts.ClearSecret(ctx, accountID, time.Now())
	if err != nil {
		return nil, err
	}
	return providerAccountModel(account), nil
}

func (r *Resolver) startProviderAuth(
	ctx context.Context,
	input model.StartProviderAuthAttemptInput,
) (*model.ProviderAuthAttempt, error) {
	service := r.providerAuth[input.ProviderKind]
	if service == nil {
		return nil, errors.New("provider authentication is unavailable")
	}
	accountID := "provider_account:" + input.ProviderKind + ":default"
	if input.ProviderAccountID != nil {
		accountID = *input.ProviderAccountID
	}
	attempt, err := service.StartAuth(
		ctx, input.ProviderKind, accountID, providerAuthMethod(input.Method),
	)
	if err != nil {
		return nil, err
	}
	return providerAuthAttemptModel(attempt), nil
}

func (r *Resolver) providerAuthAttempt(attemptID string) *model.ProviderAuthAttempt {
	for _, kind := range []string{"openrouter", "codex"} {
		service := r.providerAuth[kind]
		if service == nil {
			continue
		}
		if attempt, exists := service.Attempt(attemptID); exists {
			return providerAuthAttemptModel(attempt)
		}
	}
	return nil
}

func (r *Resolver) cancelProviderAuth(attemptID string) *model.ProviderAuthAttempt {
	for _, kind := range []string{"openrouter", "codex"} {
		service := r.providerAuth[kind]
		if service == nil {
			continue
		}
		if attempt, exists := service.Cancel(attemptID); exists {
			return providerAuthAttemptModel(attempt)
		}
	}
	return nil
}

func (r *Resolver) providerAuthEvents(
	ctx context.Context,
	attemptID string,
) (<-chan *model.ProviderAuthAttempt, error) {
	service := r.providerAuthServiceForAttempt(attemptID)
	if service == nil {
		return nil, provider.ErrAuthAttemptNotCurrent
	}
	source, err := service.Subscribe(ctx, attemptID)
	if err != nil {
		return nil, err
	}
	events := make(chan *model.ProviderAuthAttempt)
	go func() {
		defer close(events)
		for attempt := range source {
			select {
			case events <- providerAuthAttemptModel(attempt):
			case <-ctx.Done():
				return
			}
		}
	}()
	return events, nil
}

func (r *Resolver) providerAuthServiceForAttempt(attemptID string) providerAuthService {
	for _, kind := range []string{"openrouter", "codex"} {
		service := r.providerAuth[kind]
		if service == nil {
			continue
		}
		if _, exists := service.Attempt(attemptID); exists {
			return service
		}
	}
	return nil
}

func providerAccountModel(account provider.Account) *model.ProviderAccount {
	result := &model.ProviderAccount{
		ProviderAccountID: account.ID, ProviderKind: account.ProviderKind,
		AccountKey: account.AccountKey, DisplayName: account.DisplayName,
		AuthMethod: string(account.AuthMethod), Status: providerAccountStatusModel(account.Status),
		IsActive: account.IsActive, IsDefault: account.IsDefault,
		Capabilities: providerCapabilitiesModel(account),
	}
	result.LastCheckedAt = formattedTime(account.LastCheckedAt)
	result.LastAuthenticatedAt = formattedTime(account.LastAuthenticatedAt)
	result.LastErrorCode = optionalString(account.LastErrorCode)
	result.LastErrorMessage = optionalString(account.LastErrorMessage)
	return result
}

func providerCapabilitiesModel(account provider.Account) []*model.ProviderCapability {
	capabilities := provider.Capabilities(account)
	result := make([]*model.ProviderCapability, 0, len(capabilities))
	for _, capability := range capabilities {
		result = append(result, &model.ProviderCapability{
			CapabilityID: capability.ID, Status: capability.Status,
			ReliabilityContract: capability.ReliabilityContract, DataFlowClass: capability.DataFlowClass,
			Features: &model.CapabilityFeatures{
				Citations: capability.Features.Citations, DirectURLFetch: capability.Features.DirectURLFetch,
				JsRendering:          capability.Features.JSRendering,
				AuthenticatedContext: capability.Features.AuthenticatedContext,
				ResultPersistence:    capability.Features.ResultPersistence,
			},
		})
	}
	return result
}

func providerAuthAttemptModel(attempt provider.AuthAttempt) *model.ProviderAuthAttempt {
	return &model.ProviderAuthAttempt{
		AttemptID: attempt.ID, ProviderKind: attempt.ProviderKind,
		ProviderAccountID: attempt.ProviderAccountID,
		Method:            providerAuthMethodModel(attempt.Method), Status: providerAuthStatusModel(attempt.Status),
		VerificationURL: optionalString(attempt.VerificationURL), UserCode: optionalString(attempt.UserCode),
		Instructions: optionalString(attempt.Instructions), ErrorCode: optionalString(attempt.ErrorCode),
		ErrorMessage: optionalString(attempt.ErrorMessage),
	}
}

func providerAuthMethod(method model.ProviderAuthMethod) provider.AuthMethod {
	switch method {
	case model.ProviderAuthMethodOauthDeviceCode:
		return provider.AuthOAuthDeviceCode
	case model.ProviderAuthMethodOauthPkce:
		return provider.AuthOAuthPKCE
	case model.ProviderAuthMethodSecretInput:
		return provider.AuthSecretInput
	case model.ProviderAuthMethodExternalManual:
		return provider.AuthExternalManual
	case model.ProviderAuthMethodNone:
		return provider.AuthNone
	default:
		return ""
	}
}

func providerAuthMethodModel(method provider.AuthMethod) model.ProviderAuthMethod {
	switch method {
	case provider.AuthOAuthDeviceCode:
		return model.ProviderAuthMethodOauthDeviceCode
	case provider.AuthOAuthPKCE:
		return model.ProviderAuthMethodOauthPkce
	case provider.AuthSecretInput:
		return model.ProviderAuthMethodSecretInput
	case provider.AuthExternalManual:
		return model.ProviderAuthMethodExternalManual
	default:
		return model.ProviderAuthMethodNone
	}
}

func providerAccountStatusModel(status provider.AccountStatus) model.ProviderAccountStatus {
	switch status {
	case provider.StatusChecking:
		return model.ProviderAccountStatusChecking
	case provider.StatusAuthenticated:
		return model.ProviderAccountStatusAuthenticated
	case provider.StatusUnauthenticated:
		return model.ProviderAccountStatusUnauthenticated
	case provider.StatusUnavailable:
		return model.ProviderAccountStatusUnavailable
	default:
		return model.ProviderAccountStatusUnknown
	}
}

func providerAuthStatusModel(status provider.AuthAttemptStatus) model.ProviderAuthAttemptStatus {
	switch status {
	case provider.AuthAttemptCompleted:
		return model.ProviderAuthAttemptStatusCompleted
	case provider.AuthAttemptFailed:
		return model.ProviderAuthAttemptStatusFailed
	case provider.AuthAttemptExpired:
		return model.ProviderAuthAttemptStatusExpired
	case provider.AuthAttemptCancelled:
		return model.ProviderAuthAttemptStatusCancelled
	default:
		return model.ProviderAuthAttemptStatusWaitingForUser
	}
}

func formattedTime(value *time.Time) *string {
	if value == nil {
		return nil
	}
	formatted := value.UTC().Format(time.RFC3339Nano)
	return &formatted
}

func optionalString(value string) *string {
	if value == "" {
		return nil
	}
	return &value
}
