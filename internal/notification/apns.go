package notification

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/store"
)

const apnsBlocker = "APNs provider is not configured"

// ClientNotificationStatus describes one native client's alert capability.
type ClientNotificationStatus struct {
	Available   bool
	Blocker     *string
	Enabled     bool
	Environment *store.APNSEnvironment
}

// ClientLiveActivityStatus describes one native client's ActivityKit capability.
type ClientLiveActivityStatus struct {
	Available   bool
	Blocker     *string
	Enabled     bool
	Registered  bool
	Environment *store.APNSEnvironment
}

// ClientNotificationStatus returns one caller-owned registration state.
func (s *Service) ClientNotificationStatus(ctx context.Context, clientID string) (ClientNotificationStatus, error) {
	credential, err := readAPNSCredential(s.paths.APNSProvider())
	if err != nil {
		return ClientNotificationStatus{}, err
	}
	registration, err := s.database.ClientNotificationRegistration(ctx, clientID)
	if err != nil {
		return ClientNotificationStatus{}, err
	}
	status := ClientNotificationStatus{Available: credential.Configured, Enabled: registration != nil}
	if !credential.Configured {
		blocker := apnsBlocker
		status.Blocker = &blocker
	}
	if registration != nil {
		environment := registration.Environment
		status.Environment = &environment
	}
	return status, nil
}

// RegisterClientNotifications validates and stores one caller-owned device token.
func (s *Service) RegisterClientNotifications(ctx context.Context, clientID, encoded string, environment store.APNSEnvironment) (ClientNotificationStatus, error) {
	token, err := decodeAPNSToken(encoded)
	if err != nil {
		return ClientNotificationStatus{}, errors.New("device token is invalid")
	}
	if err := s.database.RegisterClientNotifications(ctx, clientID, token, environment, time.Now()); err != nil {
		return ClientNotificationStatus{}, err
	}
	return s.ClientNotificationStatus(ctx, clientID)
}

// DisableClientNotifications removes one caller-owned registration.
func (s *Service) DisableClientNotifications(ctx context.Context, clientID string) (ClientNotificationStatus, error) {
	if err := s.database.DisableClientNotifications(ctx, clientID); err != nil {
		return ClientNotificationStatus{}, err
	}
	return s.ClientNotificationStatus(ctx, clientID)
}

// ClientLiveActivityStatus returns one caller-owned ActivityKit state.
func (s *Service) ClientLiveActivityStatus(ctx context.Context, clientID string) (ClientLiveActivityStatus, error) {
	credential, err := readAPNSCredential(s.paths.APNSProvider())
	if err != nil {
		return ClientLiveActivityStatus{}, err
	}
	registration, err := s.database.ClientLiveActivityRegistration(ctx, clientID)
	if err != nil {
		return ClientLiveActivityStatus{}, err
	}
	status := ClientLiveActivityStatus{Available: credential.Configured, Enabled: registration == nil || registration.Enabled}
	if !credential.Configured {
		blocker := apnsBlocker
		status.Blocker = &blocker
	}
	if registration != nil {
		status.Registered = len(registration.PushToStartToken) > 0
		status.Environment = registration.Environment
	}
	return status, nil
}

// RegisterClientLiveActivities stores one caller's push-to-start token and active snapshot.
func (s *Service) RegisterClientLiveActivities(ctx context.Context, clientID, encoded string, environment store.APNSEnvironment, active []string) (ClientLiveActivityStatus, error) {
	token, err := decodeAPNSToken(encoded)
	if err != nil {
		return ClientLiveActivityStatus{}, errors.New("Live Activity token is invalid")
	}
	if err := s.database.RegisterClientLiveActivities(ctx, clientID, token, environment, active, time.Now()); err != nil {
		return ClientLiveActivityStatus{}, err
	}
	s.wakeDelivery()
	return s.ClientLiveActivityStatus(ctx, clientID)
}

// RegisterClientLiveActivityUpdate stores one update token for an observed active identifier.
func (s *Service) RegisterClientLiveActivityUpdate(ctx context.Context, clientID, activityID, encoded string) (bool, error) {
	token, err := decodeAPNSToken(encoded)
	if err != nil {
		return false, errors.New("Live Activity token is invalid")
	}
	changed, err := s.database.RegisterClientLiveActivityUpdate(ctx, clientID, activityID, token, time.Now())
	if changed {
		s.wakeDelivery()
	}
	return changed, err
}

// DismissClientLiveActivity removes one caller-owned active identifier.
func (s *Service) DismissClientLiveActivity(ctx context.Context, clientID, activityID string) (bool, error) {
	changed, err := s.database.DismissClientLiveActivity(ctx, clientID, activityID)
	if changed {
		s.wakeDelivery()
	}
	return changed, err
}

// DisableClientLiveActivities clears caller-owned ActivityKit token material.
func (s *Service) DisableClientLiveActivities(ctx context.Context, clientID string) (ClientLiveActivityStatus, error) {
	if err := s.database.DisableClientLiveActivities(ctx, clientID, time.Now()); err != nil {
		return ClientLiveActivityStatus{}, err
	}
	s.wakeDelivery()
	return s.ClientLiveActivityStatus(ctx, clientID)
}

func (s *Service) wakeDelivery() {
	select {
	case s.wake <- struct{}{}:
	default:
	}
}

// ClientPresence holds one focused-Chat lease until its context ends.
func (s *Service) ClientPresence(ctx context.Context, clientID string) (<-chan struct{}, error) {
	registration, err := s.database.ClientNotificationRegistration(ctx, clientID)
	if err != nil {
		return nil, err
	}
	if registration == nil {
		return nil, errors.New("client notification registration is unavailable")
	}
	s.mu.Lock()
	s.clients[clientID]++
	s.mu.Unlock()
	ready := make(chan struct{}, 1)
	ready <- struct{}{}
	go func() {
		<-ctx.Done()
		s.mu.Lock()
		s.clients[clientID]--
		if s.clients[clientID] == 0 {
			delete(s.clients, clientID)
		}
		s.mu.Unlock()
		close(ready)
	}()
	return ready, nil
}

func (s *Service) drainAPNS(ctx context.Context) error {
	for {
		value, err := s.database.ClaimDueAPNSDelivery(ctx, time.Now())
		if err != nil || value == nil {
			return err
		}
		s.mu.Lock()
		visible := s.clients[value.Registration.ClientID] > 0
		s.mu.Unlock()
		outcome, code := store.APNSSuppressed, ""
		apnsID := ""
		var revision int
		if !visible || value.Notification.Route != "chat" {
			outcome, code, apnsID, revision = s.sendAPNS(ctx, *value)
			if code != "" {
				_ = s.recordAPNSError(code, revision)
			}
		}
		if err := s.database.FinishAPNSDelivery(ctx, *value, outcome, code, apnsID, time.Now()); err != nil {
			return err
		}
	}
}

func (s *Service) sendAPNS(ctx context.Context, value store.APNSDelivery) (store.APNSDeliveryOutcome, string, string, int) {
	credential, err := readAPNSCredential(s.paths.APNSProvider())
	if err != nil {
		return store.APNSRetry, "provider_unavailable", "", 0
	}
	if !credential.Configured {
		return store.APNSFailed, "provider_unconfigured", "", credential.Revision
	}
	private, err := parseAPNSKey(*credential.PrivateKeyPEM)
	if err != nil {
		return store.APNSFailed, "provider_key_invalid", "", credential.Revision
	}
	token, err := makeAPNSJWT(private, *credential.TeamID, *credential.KeyID, time.Now())
	if err != nil {
		return store.APNSFailed, "provider_token_failed", "", credential.Revision
	}
	payload := map[string]any{"aps": map[string]any{"alert": map[string]string{
		"title": value.Notification.Title, "body": value.Notification.Body}, "sound": "default"},
		"route": value.Notification.Route, "version": 1, "eventKey": value.Notification.EventKey,
		"clientId": value.Registration.ClientID}
	if value.Notification.TaskID != nil {
		payload["taskId"] = *value.Notification.TaskID
	}
	body, err := marshalAPNSPayload(payload)
	if err != nil {
		return store.APNSFailed, "invalid_payload", "", credential.Revision
	}
	host := "api.push.apple.com"
	if value.Registration.Environment == store.APNSDevelopment {
		host = "api.sandbox.push.apple.com"
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost,
		"https://"+host+"/3/device/"+hex.EncodeToString(value.Registration.DeviceToken), bytes.NewReader(body))
	if err != nil {
		return store.APNSFailed, "invalid_request", "", credential.Revision
	}
	expires := int64(0)
	if value.Notification.TTLSeconds > 0 {
		expires = value.CreatedAt.Unix() + int64(value.Notification.TTLSeconds)
		if expires <= time.Now().Unix() {
			return store.APNSFailed, "delivery_expired", "", credential.Revision
		}
	}
	request.Header.Set("authorization", "bearer "+token)
	request.Header.Set("content-type", "application/json")
	request.Header.Set("apns-topic", apnsTopic)
	request.Header.Set("apns-push-type", "alert")
	request.Header.Set("apns-priority", map[bool]string{true: "10", false: "5"}[value.Notification.Urgency == "high"])
	request.Header.Set("apns-expiration", fmt.Sprint(expires))
	response, err := s.apns.Do(request)
	if err != nil {
		return store.APNSRetry, "transport_unavailable", "", credential.Revision
	}
	defer response.Body.Close()
	responseBody, _ := io.ReadAll(io.LimitReader(response.Body, 4097))
	apnsID := response.Header.Get("apns-id")
	if len(apnsID) > 128 || !isASCII(apnsID) {
		apnsID = ""
	}
	outcome, code := apnsResponseOutcome(response.StatusCode, responseBody)
	return outcome, code, apnsID, credential.Revision
}

func marshalAPNSPayload(value any) ([]byte, error) {
	var buffer bytes.Buffer
	encoder := json.NewEncoder(&buffer)
	encoder.SetEscapeHTML(false)
	if err := encoder.Encode(value); err != nil {
		return nil, err
	}
	body := bytes.TrimSuffix(buffer.Bytes(), []byte{'\n'})
	if len(body) > 4096 {
		return nil, errors.New("APNs payload exceeds 4096 bytes")
	}
	return body, nil
}

func apnsResponseOutcome(status int, body []byte) (store.APNSDeliveryOutcome, string) {
	if status >= 200 && status < 300 {
		return store.APNSDelivered, ""
	}
	if status == http.StatusBadRequest || status == http.StatusGone {
		var rejection struct {
			Reason string `json:"reason"`
		}
		if len(body) <= 4096 {
			_ = json.Unmarshal(body, &rejection)
		}
		if rejection.Reason == "BadDeviceToken" || rejection.Reason == "DeviceTokenNotForTopic" || rejection.Reason == "Unregistered" {
			return store.APNSInvalid, ""
		}
		return store.APNSFailed, "remote_rejected"
	}
	if status == http.StatusForbidden {
		return store.APNSFailed, "provider_auth_rejected"
	}
	if status == http.StatusTooManyRequests || status >= 500 {
		return store.APNSRetry, "remote_retry"
	}
	return store.APNSFailed, "remote_rejected"
}

func makeAPNSJWT(private *ecdsa.PrivateKey, teamID, keyID string, now time.Time) (string, error) {
	header, _ := json.Marshal(map[string]string{"alg": "ES256", "kid": keyID})
	claims, _ := json.Marshal(map[string]any{"iss": teamID, "iat": now.Unix()})
	unsigned := base64.RawURLEncoding.EncodeToString(header) + "." + base64.RawURLEncoding.EncodeToString(claims)
	digest := sha256.Sum256([]byte(unsigned))
	r, ss, err := ecdsa.Sign(rand.Reader, private, digest[:])
	if err != nil {
		return "", err
	}
	signature := make([]byte, 64)
	r.FillBytes(signature[:32])
	ss.FillBytes(signature[32:])
	return unsigned + "." + base64.RawURLEncoding.EncodeToString(signature), nil
}

func decodeAPNSToken(value string) ([]byte, error) {
	if value == "" || len(value) > 1366 || strings.TrimSpace(value) != value {
		return nil, errors.New("invalid token")
	}
	token, err := base64.RawURLEncoding.Strict().DecodeString(value)
	if err != nil || len(token) < 1 || len(token) > 1024 {
		return nil, errors.New("invalid token")
	}
	return token, nil
}

func isASCII(value string) bool {
	for _, b := range []byte(value) {
		if b > 127 {
			return false
		}
	}
	return true
}
