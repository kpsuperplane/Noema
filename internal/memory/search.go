package memory

import (
	"slices"
	"strings"
	"unicode"

	"github.com/rivo/uniseg"
)

const maxSearchResults = 16
const maxRelevantSearchTerms = 128

type searchPage struct {
	result                SearchResult
	titleTerms, bodyTerms []string
	bodyWords             []string
}

type scoredSearchResult struct {
	result SearchResult
	score  int
}

// Search finds pages that contain every query term as a lexical prefix.
func (s *Store) Search(query string, limit int) ([]SearchResult, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	terms := lexicalTerms(query)
	if len(terms) == 0 || limit < 1 {
		return []SearchResult{}, nil
	}
	limit = min(limit, maxSearchResults)
	matches := make([]scoredSearchResult, 0)
	for _, page := range s.searchPages {
		score := 0
		matched := true
		for _, term := range terms {
			titleCount := lexicalPrefixCount(page.titleTerms, term)
			bodyCount := lexicalPrefixCount(page.bodyTerms, term)
			if titleCount+bodyCount == 0 {
				matched = false
				break
			}
			score += titleCount*4 + bodyCount
		}
		if !matched {
			continue
		}
		result := page.result
		result.Snippet = lexicalSnippet(page.bodyWords, terms)
		matches = append(matches, scoredSearchResult{result: result, score: score})
	}
	slices.SortFunc(matches, func(left, right scoredSearchResult) int {
		if left.score != right.score {
			return right.score - left.score
		}
		return strings.Compare(left.result.Path, right.result.Path)
	})
	if len(matches) > limit {
		matches = matches[:limit]
	}
	results := make([]SearchResult, len(matches))
	for index := range matches {
		results[index] = matches[index].result
	}
	return results, nil
}

// SearchRelevant ranks pages that contain any term from a long evidence query.
func (s *Store) SearchRelevant(query string, limit int) ([]SearchResult, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	terms := uniqueRelevantTerms(lexicalTerms(query))
	if len(terms) == 0 || limit < 1 {
		return []SearchResult{}, nil
	}
	limit = min(limit, maxSearchResults)
	matches := make([]scoredSearchResult, 0)
	for _, page := range s.searchPages {
		score := 0
		for _, term := range terms {
			titleCount := lexicalTermCount(page.titleTerms, term)
			bodyCount := lexicalTermCount(page.bodyTerms, term)
			score += titleCount*4 + bodyCount
		}
		if score == 0 {
			continue
		}
		result := page.result
		result.Snippet = lexicalSnippet(page.bodyWords, terms)
		matches = append(matches, scoredSearchResult{result: result, score: score})
	}
	slices.SortFunc(matches, func(left, right scoredSearchResult) int {
		if left.score != right.score {
			return right.score - left.score
		}
		return strings.Compare(left.result.Path, right.result.Path)
	})
	if len(matches) > limit {
		matches = matches[:limit]
	}
	results := make([]SearchResult, len(matches))
	for index := range matches {
		results[index] = matches[index].result
	}
	return results, nil
}

func uniqueRelevantTerms(terms []string) []string {
	seen := make(map[string]bool, len(terms))
	unique := make([]string, 0, min(len(terms), maxRelevantSearchTerms))
	for _, term := range terms {
		if !seen[term] {
			seen[term] = true
			unique = append(unique, term)
		}
	}
	if len(unique) <= maxRelevantSearchTerms {
		return unique
	}
	bounded := make([]string, 0, maxRelevantSearchTerms)
	bounded = append(bounded, unique[:maxRelevantSearchTerms/2]...)
	bounded = append(bounded, unique[len(unique)-maxRelevantSearchTerms/2:]...)
	return bounded
}

func buildSearchPages(pages []Page) []searchPage {
	result := make([]searchPage, len(pages))
	for index, page := range pages {
		result[index] = searchPage{
			result:     SearchResult{ID: page.ID, Path: page.Path, Title: page.Title, Hash: page.Hash},
			titleTerms: lexicalTerms(page.Title), bodyTerms: lexicalTerms(page.Body),
			bodyWords: lexicalWords(page.Body, false),
		}
	}
	return result
}

func lexicalTerms(value string) []string {
	return lexicalWords(value, true)
}

func lexicalWords(value string, lower bool) []string {
	terms := make([]string, 0)
	state := -1
	for value != "" {
		word, rest, next := uniseg.FirstWordInString(value, state)
		term := strings.TrimFunc(word, func(character rune) bool {
			return !unicode.IsLetter(character) && !unicode.IsNumber(character)
		})
		if lower {
			term = strings.ToLower(term)
		}
		if term != "" {
			terms = append(terms, term)
		}
		value, state = rest, next
	}
	return terms
}

func lexicalPrefixCount(words []string, prefix string) int {
	count := 0
	for _, word := range words {
		if strings.HasPrefix(word, prefix) {
			count++
		}
	}
	return count
}

func lexicalTermCount(words []string, term string) int {
	count := 0
	for _, word := range words {
		if word == term {
			count++
		}
	}
	return count
}

func lexicalSnippet(words, queries []string) string {
	if len(words) == 0 {
		return ""
	}
	match := 0
	for index, word := range words {
		normalized := strings.ToLower(word)
		if slices.ContainsFunc(queries, func(query string) bool { return strings.HasPrefix(normalized, query) }) {
			match = index
			break
		}
	}
	start := max(0, match-8)
	end := min(len(words), start+18)
	if end-start < 18 {
		start = max(0, end-18)
	}
	snippet := strings.Join(words[start:end], " ")
	if start > 0 {
		snippet = "… " + snippet
	}
	if end < len(words) {
		snippet += " …"
	}
	return snippet
}
