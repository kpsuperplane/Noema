package main

import (
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
)

func main() {
	port := 0
	for index := 1; index+1 < len(os.Args); index++ {
		if os.Args[index] != "--port" {
			continue
		}
		parsed, err := strconv.Atoi(os.Args[index+1])
		if err != nil {
			fmt.Fprintln(os.Stderr, "invalid --port")
			os.Exit(2)
		}
		port = parsed
		break
	}
	if port < 1 || port > 65535 {
		fmt.Fprintln(os.Stderr, "missing --port")
		os.Exit(2)
	}

	mux := http.NewServeMux()
	mux.HandleFunc("/health", func(writer http.ResponseWriter, request *http.Request) {
		if request.Method != http.MethodGet {
			http.NotFound(writer, request)
			return
		}
		_, _ = io.WriteString(writer, "OK")
	})
	mux.HandleFunc("/v1/chat/completions", func(writer http.ResponseWriter, request *http.Request) {
		if request.Method != http.MethodPost {
			http.NotFound(writer, request)
			return
		}
		_, _ = io.Copy(io.Discard, request.Body)
		_ = request.Body.Close()
		body := `data: {"id":"response","model":"rust-host-local-model","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call-1","function":{"name":"noema_local_qualification","arguments":"{}"}}]}}]}

data: [DONE]

`
		writer.Header().Set("Content-Type", "text/event-stream")
		writer.Header().Set("Content-Length", strconv.Itoa(len(body)))
		_, _ = io.WriteString(writer, body)
	})
	if err := http.ListenAndServe("127.0.0.1:"+strconv.Itoa(port), mux); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
