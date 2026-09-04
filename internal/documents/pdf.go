package documents

import (
	"bytes"
	"context"
	"errors"
	"time"

	"github.com/giraffesyo/pdf"
)

const documentPDFTimeout = 30 * time.Second

var errPDFComplete = errors.New("PDF output complete")

func parsePDF(data []byte) (string, error) {
	ctx, cancel := context.WithTimeout(context.Background(), documentPDFTimeout)
	defer cancel()

	var output markdownDocument
	_, err := pdf.ExtractPages(ctx, bytes.NewReader(data), int64(len(data)), pdf.Options{
		Concurrency: 1,
		Limits: pdf.Limits{
			MaxStreamBytes:       maxDocumentPartBytes,
			MaxOperatorsPerPage:  500_000,
			MaxGlyphsPerPage:     100_000,
			MaxFormDepth:         8,
			MaxImagesPerPage:     10_000,
			MaxImageBytesPerPage: 1,
			MaxImagePixels:       1,
		},
	}, func(page pdf.Page) error {
		output.block("", page.Text())
		if output.count >= maxPreviewCharacters {
			return errPDFComplete
		}
		return nil
	})
	if err != nil && !errors.Is(err, errPDFComplete) {
		return "", errInvalidDocument
	}
	result := output.String()
	if result == "" {
		return "", errInvalidDocument
	}
	return result, nil
}
