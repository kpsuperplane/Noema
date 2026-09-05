package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"

	"github.com/kpsuperplane/noema/internal/modeleval"
)

func main() {
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt)
	defer cancel()
	if err := modeleval.Run(ctx, ".", os.Args[1:]); err != nil {
		fmt.Fprintln(os.Stderr, "noema-model-evals:", err)
		os.Exit(1)
	}
}
