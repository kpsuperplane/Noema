package graphql

import (
	"context"
	"errors"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) primaryConversation(ctx context.Context) (*model.PrimaryConversation, error) {
	conversation, err := r.Store.PrimaryConversation(ctx)
	if err != nil || conversation == nil {
		return nil, err
	}
	return primaryConversationModel(*conversation), nil
}

func (r *Resolver) conversationTranscriptPage(
	ctx context.Context,
	input model.ConversationTranscriptPageInput,
) (*model.ConversationTranscriptPage, error) {
	limit := 80
	if input.Limit != nil {
		limit = *input.Limit
	}
	if limit < 1 || limit > 200 {
		return nil, errors.New("conversationTranscriptPage limit must be within 1..200")
	}
	if _, err := r.Store.Conversation(ctx, input.ConversationID); err != nil {
		return nil, err
	}
	return emptyTranscriptPage(), nil
}

func (r *Resolver) conversationEvents(
	ctx context.Context,
	conversationID string,
) (<-chan model.ConversationEvent, error) {
	if _, err := r.Store.Conversation(ctx, conversationID); err != nil {
		return nil, err
	}
	events := make(chan model.ConversationEvent, 1)
	events <- model.SubscriptionReadyEvent{ConversationID: conversationID}
	go func() {
		defer close(events)
		<-ctx.Done()
	}()
	return events, nil
}

func primaryConversationModel(conversation store.Conversation) *model.PrimaryConversation {
	return &model.PrimaryConversation{
		ConversationID:       conversation.ID,
		Provider:             conversation.Provider,
		LatestTranscriptPage: emptyTranscriptPage(),
	}
}

func emptyTranscriptPage() *model.ConversationTranscriptPage {
	return &model.ConversationTranscriptPage{
		Items:    []*model.ConversationItem{},
		PageInfo: &model.ConversationTranscriptPageInfo{HasMoreBefore: false},
	}
}
