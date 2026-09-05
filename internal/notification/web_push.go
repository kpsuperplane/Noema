// Package notification owns browser Web Push registration, presence, and delivery.
package notification

import (
	"bytes"
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"encoding/json"
	"encoding/pem"
	"errors"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"os"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	webpush "github.com/ergochat/webpush-go/v2"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/netpolicy"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/yuin/goldmark"
	markdownast "github.com/yuin/goldmark/ast"
	markdowntext "github.com/yuin/goldmark/text"
)

const (
	localHumanID          = "human:local"
	vapidFileLimit        = 4096
	recoveryInterval      = 30 * time.Second
	pushRequestTimeout    = 30 * time.Second
	pushConnectionTimeout = 10 * time.Second
)

// Service owns one installation's Web Push signing key and visibility leases.
type Service struct {
	database *store.Store
	paths    home.Paths
	origin   string
	keys     *webpush.VAPIDKeys
	apns     *http.Client
	visible  map[string]int
	clients  map[string]int
	mu       sync.Mutex
	wake     chan struct{}
}

type vapidFile struct {
	Version       int    `json:"version"`
	PrivateKeyPEM string `json:"private_key_pem"`
}

// New creates Web Push support. HTTP origins remain unavailable without creating a key.
func New(paths home.Paths, database *store.Store, publicOrigin string) (*Service, error) {
	if database == nil {
		return nil, errors.New("Web Push store is unavailable")
	}
	parsed, err := url.Parse(publicOrigin)
	if err != nil || parsed.Hostname() == "" {
		return nil, errors.New("invalid Web Push origin")
	}
	service := &Service{database: database, paths: paths, origin: publicOrigin,
		visible: make(map[string]int), clients: make(map[string]int), wake: make(chan struct{}, 1),
		apns: &http.Client{Timeout: pushRequestTimeout, Transport: &http.Transport{
			Proxy: nil, ForceAttemptHTTP2: true, DialContext: (&net.Dialer{Timeout: pushConnectionTimeout}).DialContext,
		}}}
	if parsed.Scheme != "https" {
		return service, nil
	}
	service.keys, err = loadOrCreateVAPID(paths.WebPushVAPID())
	if err != nil {
		return nil, err
	}
	return service, nil
}

// Available reports whether the public origin permits Web Push.
func (s *Service) Available() bool { return s != nil && s.keys != nil }

// ApplicationServerKey returns the public VAPID key only.
func (s *Service) ApplicationServerKey() string {
	if !s.Available() {
		return ""
	}
	return s.keys.PublicKeyString()
}

// Status returns this session's opaque registration identifier for one endpoint.
func (s *Service) Status(
	ctx context.Context, session [32]byte, endpoint string,
) (string, error) {
	if endpoint == "" {
		return "", nil
	}
	if len(endpoint) > 2048 {
		return "", errors.New("Web Push endpoint is unavailable")
	}
	return s.database.WebPushSubscriptionID(ctx, localHumanID, session, endpoint)
}

// Register validates and stores one browser Push capability.
func (s *Service) Register(
	ctx context.Context, session [32]byte, endpoint, p256dh, auth string,
) (string, error) {
	if !s.Available() {
		return "", errors.New("Web Push requires an HTTPS public origin")
	}
	normalized, err := validatePushEndpoint(endpoint)
	if err != nil {
		return "", err
	}
	if _, err := webpush.DecodeSubscriptionKeys(auth, p256dh); err != nil {
		return "", errors.New("Web Push subscription is invalid")
	}
	value, err := s.database.RegisterWebPushSubscription(ctx, store.NewWebPushSubscription{
		OwnerHumanID: localHumanID, SessionHash: session, Endpoint: normalized,
		P256DH: p256dh, AuthSecret: auth,
	}, time.Now())
	if err != nil {
		return "", err
	}
	return value.ID, nil
}

// Remove deletes one registration only when the current session owns it.
func (s *Service) Remove(ctx context.Context, session [32]byte, id string) (bool, error) {
	return s.database.RemoveWebPushSubscription(ctx, localHumanID, session, id)
}

// Presence holds one focused-Chat visibility lease until its context ends.
func (s *Service) Presence(ctx context.Context, session [32]byte, id string) (<-chan struct{}, error) {
	owned, err := s.database.OwnsWebPushSubscription(ctx, localHumanID, session, id)
	if err != nil {
		return nil, err
	}
	if !owned {
		return nil, errors.New("Web Push subscription is unavailable")
	}
	s.mu.Lock()
	s.visible[id]++
	s.mu.Unlock()
	ready := make(chan struct{}, 1)
	ready <- struct{}{}
	go func() {
		<-ctx.Done()
		s.mu.Lock()
		s.visible[id]--
		if s.visible[id] == 0 {
			delete(s.visible, id)
		}
		s.mu.Unlock()
		close(ready)
	}()
	return ready, nil
}

// QueueTaskAttention queues one bounded future Task-attention notification.
func (s *Service) QueueTaskAttention(
	ctx context.Context, eventKey, title, body, taskID, taskPath string,
) error {
	value := store.WebPushNotification{EventKey: eventKey,
		Title: notificationText(title), Body: notificationText(body), NavigatePath: taskPath,
		Urgency: "high", TTLSeconds: 86400}
	now := time.Now()
	var webErr error
	if s.Available() {
		webErr = s.database.QueueWebPushNotification(ctx, value, nil, now)
	}
	apnsErr := s.database.QueueAPNSNotification(ctx, store.APNSNotification{
		EventKey: eventKey, Title: value.Title, Body: value.Body, Route: "task",
		TaskID: &taskID, Urgency: "high", TTLSeconds: 86400,
	}, nil, now)
	select {
	case s.wake <- struct{}{}:
	default:
	}
	return errors.Join(webErr, apnsErr)
}

// Run projects primary Chat final answers and delivers due notifications.
func (s *Service) Run(ctx context.Context, events <-chan runtime.Event) {
	defer s.apns.CloseIdleConnections()
	for {
		projectionErr := s.reconcilePrimary(ctx)
		var deliveryErr error
		if s.Available() {
			deliveryErr = s.drain(ctx)
		}
		apnsErr := s.drainAPNS(ctx)
		deadline, err := s.nextDelivery(ctx)
		delay := recoveryInterval
		failed := projectionErr != nil || deliveryErr != nil || apnsErr != nil || err != nil
		if !failed && deadline != nil {
			until := time.Until(*deadline)
			if until < delay {
				delay = max(until, 0)
			}
		}
		timer := time.NewTimer(delay)
		if failed {
			select {
			case <-ctx.Done():
				stopTimer(timer)
				return
			case <-timer.C:
			}
			continue
		}
		select {
		case <-ctx.Done():
			stopTimer(timer)
			return
		case _, ok := <-events:
			stopTimer(timer)
			if !ok {
				events = nil
			}
		case <-s.wake:
			stopTimer(timer)
		case <-timer.C:
		}
	}
}

func (s *Service) nextDelivery(ctx context.Context) (*time.Time, error) {
	apns, err := s.database.NextAPNSDelivery(ctx)
	if err != nil || !s.Available() {
		return apns, err
	}
	web, err := s.database.NextWebPushDelivery(ctx)
	if err != nil || apns == nil {
		return web, err
	}
	if web != nil && web.Before(*apns) {
		return web, nil
	}
	return apns, nil
}

func (s *Service) reconcilePrimary(ctx context.Context) error {
	conversationID, latest, err := s.database.WebPushPrimarySource(ctx)
	if err != nil || conversationID == "" {
		return err
	}
	checkpoint, err := s.database.WebPushPrimaryCheckpoint(ctx)
	if err != nil {
		return err
	}
	if checkpoint.ConversationID != conversationID {
		return s.database.AdvanceWebPushPrimaryCheckpoint(ctx, conversationID, latest, time.Now())
	}
	for {
		items, err := s.database.WebPushPrimaryItems(ctx, conversationID, checkpoint.Sequence, 100)
		if err != nil {
			return err
		}
		if len(items) == 0 {
			return nil
		}
		for _, item := range items {
			if item.Kind == store.ConversationAssistantText && item.Metadata["phase"] == "final_answer" && item.TurnID != "" {
				agent, agentErr := s.database.Agent(ctx, store.PrimaryAgentID)
				title := "Noema"
				if agentErr == nil && agent.DisplayName != nil {
					title = *agent.DisplayName
				}
				if err := s.queue(ctx, store.WebPushNotification{EventKey: "chat-turn:" + item.TurnID,
					Title: notificationText(title), Body: notificationText(item.ContentText),
					NavigatePath: "/", Urgency: "normal", TTLSeconds: 3600}); err != nil {
					return err
				}
			}
			checkpoint.Sequence = item.Sequence
		}
		if err := s.database.AdvanceWebPushPrimaryCheckpoint(ctx, conversationID, checkpoint.Sequence, time.Now()); err != nil {
			return err
		}
		if len(items) < 100 {
			return nil
		}
	}
}

func (s *Service) queue(ctx context.Context, value store.WebPushNotification) error {
	s.mu.Lock()
	visible := make(map[string]struct{}, len(s.visible))
	for id := range s.visible {
		visible[id] = struct{}{}
	}
	s.mu.Unlock()
	now := time.Now()
	if s.Available() {
		if err := s.database.QueueWebPushNotification(ctx, value, visible, now); err != nil {
			return err
		}
	}
	s.mu.Lock()
	clients := make(map[string]struct{}, len(s.clients))
	for id := range s.clients {
		clients[id] = struct{}{}
	}
	s.mu.Unlock()
	if err := s.database.QueueAPNSNotification(ctx, store.APNSNotification{
		EventKey: value.EventKey, Title: value.Title, Body: value.Body,
		Route: "chat", Urgency: value.Urgency, TTLSeconds: value.TTLSeconds,
	}, clients, now); err != nil {
		return err
	}
	select {
	case s.wake <- struct{}{}:
	default:
	}
	return nil
}

func (s *Service) drain(ctx context.Context) error {
	for {
		value, err := s.database.ClaimDueWebPushDelivery(ctx, time.Now())
		if err != nil || value == nil {
			return err
		}
		if s.isVisible(value.Subscription.ID) {
			err = s.database.FinishWebPushDelivery(ctx, *value, store.WebPushSuppressed, "", time.Now())
		} else {
			outcome, code := s.send(ctx, *value)
			err = s.database.FinishWebPushDelivery(ctx, *value, outcome, code, time.Now())
		}
		if err != nil {
			return err
		}
	}
}

func (s *Service) send(ctx context.Context, value store.WebPushDelivery) (store.WebPushDeliveryOutcome, string) {
	keys, err := webpush.DecodeSubscriptionKeys(value.Subscription.AuthSecret, value.Subscription.P256DH)
	if err != nil {
		return store.WebPushFailed, "invalid_subscription"
	}
	payload, err := json.Marshal(map[string]any{"web_push": 8030, "notification": map[string]any{
		"title": value.Notification.Title, "body": value.Notification.Body,
		"navigate": s.origin + value.Notification.NavigatePath,
		"tag":      value.Notification.EventKey, "silent": false,
	}})
	if err != nil {
		return store.WebPushFailed, "invalid_payload"
	}
	requestContext, cancel := context.WithTimeout(ctx, pushRequestTimeout)
	defer cancel()
	response, err := webpush.SendNotification(requestContext, payload,
		&webpush.Subscription{Endpoint: value.Subscription.Endpoint, Keys: keys}, &webpush.Options{
			HTTPClient: publicPushClient{}, Subscriber: s.origin, VAPIDKeys: s.keys,
			TTL: value.Notification.TTLSeconds, Urgency: webpush.Urgency(value.Notification.Urgency),
		})
	if err != nil {
		return store.WebPushRetry, "transport_unavailable"
	}
	defer response.Body.Close()
	_, _ = io.Copy(io.Discard, io.LimitReader(response.Body, 4096))
	switch {
	case response.StatusCode >= 200 && response.StatusCode < 300:
		return store.WebPushDelivered, ""
	case response.StatusCode == http.StatusNotFound || response.StatusCode == http.StatusGone:
		return store.WebPushExpired, "expired"
	case response.StatusCode == http.StatusTooManyRequests || response.StatusCode >= 500:
		return store.WebPushRetry, "remote_retry"
	default:
		return store.WebPushFailed, "remote_rejected"
	}
}

func (s *Service) isVisible(id string) bool {
	s.mu.Lock()
	defer s.mu.Unlock()
	return s.visible[id] > 0
}

func loadOrCreateVAPID(path string) (*webpush.VAPIDKeys, error) {
	data, err := home.ReadPrivateFile(path, vapidFileLimit)
	if errors.Is(err, os.ErrNotExist) {
		private, generateErr := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
		if generateErr != nil {
			return nil, errors.New("generate Web Push key")
		}
		encoded, encodeErr := x509.MarshalPKCS8PrivateKey(private)
		if encodeErr != nil {
			return nil, errors.New("encode Web Push key")
		}
		document, encodeErr := json.Marshal(vapidFile{Version: 1, PrivateKeyPEM: string(pem.EncodeToMemory(
			&pem.Block{Type: "PRIVATE KEY", Bytes: encoded}))})
		if encodeErr != nil {
			return nil, errors.New("encode Web Push configuration")
		}
		if err := home.AtomicWritePrivate(path, document); err != nil {
			return nil, fmt.Errorf("store Web Push configuration: %w", err)
		}
		data = document
	} else if err != nil {
		return nil, fmt.Errorf("read Web Push configuration: %w", err)
	}
	var document vapidFile
	if err := json.Unmarshal(data, &document); err != nil || document.Version != 1 {
		return nil, errors.New("Web Push configuration is invalid")
	}
	block, rest := pem.Decode([]byte(document.PrivateKeyPEM))
	if block == nil || block.Type != "PRIVATE KEY" || len(strings.TrimSpace(string(rest))) != 0 {
		return nil, errors.New("Web Push configuration is invalid")
	}
	parsed, err := x509.ParsePKCS8PrivateKey(block.Bytes)
	private, ok := parsed.(*ecdsa.PrivateKey)
	if err != nil || !ok || private.Curve != elliptic.P256() {
		return nil, errors.New("Web Push configuration is invalid")
	}
	keys, err := webpush.ECDSAToVAPIDKeys(private)
	if err != nil {
		return nil, errors.New("Web Push configuration is invalid")
	}
	return keys, nil
}

func validatePushEndpoint(value string) (string, error) {
	parsed, err := url.Parse(value)
	if err != nil || parsed.Scheme != "https" || parsed.Hostname() == "" || parsed.User != nil ||
		parsed.Fragment != "" || len(value) > 2048 {
		return "", errors.New("Web Push endpoint is unavailable")
	}
	host := strings.TrimSuffix(strings.ToLower(parsed.Hostname()), ".")
	if host == "localhost" || strings.HasSuffix(host, ".localhost") {
		return "", errors.New("Web Push endpoint is unavailable")
	}
	if address, parseErr := netip.ParseAddr(host); parseErr == nil && !netpolicy.IsPublic(address) {
		return "", errors.New("Web Push endpoint is unavailable")
	}
	return parsed.String(), nil
}

type publicPushClient struct{}

func (publicPushClient) Do(request *http.Request) (*http.Response, error) {
	addresses, err := netpolicy.ResolvePublic(request.Context(), request.URL.Hostname())
	if err != nil {
		return nil, errors.New("Push endpoint is unavailable")
	}
	checked := netpolicy.CheckedURL{URL: request.URL, Addresses: addresses}
	client := netpolicy.PinnedClient(checked, pushConnectionTimeout)
	return client.Do(request)
}

func notificationText(value string) string {
	value = strings.Join(strings.Fields(markdownText(value)), " ")
	if len(value) <= 600 {
		return value
	}
	end := 597
	for end > 0 && !utf8.RuneStart(value[end]) {
		end--
	}
	return strings.TrimSpace(value[:end]) + "…"
}

func markdownText(value string) string {
	source := []byte(value)
	document := goldmark.DefaultParser().Parse(markdowntext.NewReader(source))
	var plain bytes.Buffer
	_ = markdownast.Walk(document, func(node markdownast.Node, entering bool) (markdownast.WalkStatus, error) {
		if !entering {
			if node.Type() == markdownast.TypeBlock {
				plain.WriteByte(' ')
			}
			return markdownast.WalkContinue, nil
		}
		switch current := node.(type) {
		case *markdownast.HTMLBlock, *markdownast.RawHTML:
			return markdownast.WalkSkipChildren, nil
		case *markdownast.CodeBlock:
			plain.Write(current.Lines().Value(source))
			return markdownast.WalkSkipChildren, nil
		case *markdownast.FencedCodeBlock:
			plain.Write(current.Lines().Value(source))
			return markdownast.WalkSkipChildren, nil
		case *markdownast.Text:
			plain.Write(current.Segment.Value(source))
			if current.SoftLineBreak() || current.HardLineBreak() {
				plain.WriteByte(' ')
			}
		case *markdownast.String:
			plain.Write(current.Value)
		}
		return markdownast.WalkContinue, nil
	})
	return plain.String()
}

func stopTimer(timer *time.Timer) {
	if !timer.Stop() {
		select {
		case <-timer.C:
		default:
		}
	}
}
