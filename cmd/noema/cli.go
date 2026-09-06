package main

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/home"
	"github.com/urfave/cli/v3"
)

func usageError(message string) error { return cli.Exit(message, 2) }

func commandExitCode(ctx context.Context, err error) int {
	if ctx.Err() != nil {
		return 130
	}
	var code cli.ExitCoder
	if errors.As(err, &code) {
		return code.ExitCode()
	}
	return 1
}

func stringFlag(name, usage string) cli.Flag { return &cli.StringFlag{Name: name, Usage: usage} }

func cliCommand(input io.Reader, output, diagnostics io.Writer) *cli.Command {
	root := &cli.Command{Name: "noema", Usage: "Run Noema or use its private local API", Reader: input, Writer: output, ErrWriter: diagnostics,
		EnableShellCompletion: true,
		ExitErrHandler:        func(context.Context, *cli.Command, error) {},
		OnUsageError:          func(_ context.Context, _ *cli.Command, err error, _ bool) error { return usageError(err.Error()) },
		Flags: []cli.Flag{
			stringFlag("listen", "Override the server web bind address"),
			&cli.BoolFlag{Name: "desktop-sidecar", Hidden: true},
			&cli.StringFlag{Name: "socket", Usage: "Local socket path", Sources: cli.EnvVars("NOEMA_SOCKET")},
			&cli.DurationFlag{Name: "timeout", Value: 30 * time.Second, Usage: "Request timeout; streams continue until completion or interruption"},
		},
	}
	serve := func(ctx context.Context, c *cli.Command) error {
		if c.NArg() != 0 {
			return usageError("unknown command: " + c.Args().First())
		}
		return serveCommand(ctx, c.String("listen"), c.Bool("desktop-sidecar"), input, output)
	}
	root.Action = serve
	clientAction := func(fn func(context.Context, *cli.Command, *localClient) error) cli.ActionFunc {
		return func(ctx context.Context, c *cli.Command) error {
			if c.IsSet("listen") || c.Bool("desktop-sidecar") {
				return usageError("server flags require noema serve")
			}
			if runtime.GOOS == "windows" {
				return errors.New("local socket commands are unavailable on Windows")
			}
			path := c.String("socket")
			if path == "" {
				paths, err := home.Resolve()
				if err != nil {
					return err
				}
				path = filepath.Join(paths.Root(), "run", "graphql.sock")
			}
			if c.Duration("timeout") <= 0 {
				return usageError("timeout must be positive")
			}
			client := newLocalClient(path, c.Duration("timeout"), output)
			defer client.http.CloseIdleConnections()
			return fn(ctx, c, client)
		}
	}
	root.Commands = []*cli.Command{
		{Name: "serve", Usage: "Start the Noema server", Action: serve},
		{Name: "status", Usage: "Read server status", Action: clientAction(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			return l.printQuery(ctx, "{localStatus{primaryAgentDisplayName}}", nil)
		})},
		{Name: "schema", Usage: "Print the running server GraphQL schema", Action: clientAction(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			return l.schema(ctx)
		})},
		{Name: "api", Usage: "Execute GraphQL; streams emit one JSON object per line", ArgsUsage: "[DOCUMENT]",
			Flags: []cli.Flag{stringFlag("file", "Read the document from a file; - reads stdin"), stringFlag("variables", "JSON variables object"), stringFlag("variables-file", "Read JSON variables from a file; - reads stdin"), stringFlag("operation-name", "Select a named operation"), &cli.BoolFlag{Name: "subscribe", Usage: "Stream a subscription"}},
			Action: clientAction(func(ctx context.Context, c *cli.Command, l *localClient) error {
				if c.String("file") == "-" && c.String("variables-file") == "-" {
					return usageError("only one input can read stdin")
				}
				doc, err := commandText(c, input, true)
				if err != nil {
					return err
				}
				variables := map[string]any{}
				raw := c.String("variables")
				if c.IsSet("variables-file") {
					if c.IsSet("variables") {
						return usageError("choose variables or variables-file")
					}
					b, e := readCommandFile(c.String("variables-file"), input)
					if e != nil {
						return e
					}
					raw = string(b)
				}
				if c.IsSet("variables") || c.IsSet("variables-file") {
					dec := json.NewDecoder(strings.NewReader(raw))
					dec.UseNumber()
					if dec.Decode(&variables) != nil || variables == nil || dec.Decode(new(any)) != io.EOF {
						return usageError("variables must be one JSON object")
					}
				}
				request := apiRequest{Query: doc, Variables: variables, OperationName: c.String("operation-name")}
				if c.Bool("subscribe") {
					return l.stream(ctx, request, nil)
				}
				response, err := l.query(ctx, request)
				if response != nil {
					if e := l.emit(response); e != nil {
						return e
					}
				}
				return err
			})},
	}
	root.Commands = append(root.Commands, taskCommands(input, clientAction), chatCommands(input, clientAction))
	root.Commands = append(root.Commands,
		controlCommands(input, clientAction),
		proposalCommands(input, clientAction),
		interventionCommands(input, clientAction),
		actionCommands(input, clientAction),
	)
	var configure func(*cli.Command)
	configure = func(command *cli.Command) {
		command.OnUsageError = root.OnUsageError
		if command.Action == nil && len(command.Commands) > 0 {
			command.Action = func(_ context.Context, c *cli.Command) error {
				if c.NArg() > 0 {
					return usageError("unknown command: " + c.Args().First())
				}
				return cli.ShowSubcommandHelp(c)
			}
		}
		for _, child := range command.Commands {
			configure(child)
		}
	}
	configure(root)
	return root
}

func argumentCount(c *cli.Command, n int) error {
	if c.NArg() != n {
		return usageError(fmt.Sprintf("%s requires %d arguments", c.FullName(), n))
	}
	return nil
}

func readCommandFile(path string, input io.Reader) ([]byte, error) {
	if path != "-" {
		file, err := os.Open(path)
		if err != nil {
			return nil, err
		}
		defer file.Close()
		input = file
	}
	body, err := io.ReadAll(io.LimitReader(input, 64*1024+1))
	if len(body) > 64*1024 {
		return nil, usageError("input exceeds 64 KiB")
	}
	return body, err
}

func commandText(c *cli.Command, input io.Reader, required bool) (string, error) {
	if c.IsSet("file") {
		if c.NArg() != 0 {
			return "", usageError("choose a text argument or --file")
		}
		b, err := readCommandFile(c.String("file"), input)
		if err != nil {
			return "", err
		}
		if required && strings.TrimSpace(string(b)) == "" {
			return "", usageError("input cannot be empty")
		}
		return string(b), nil
	}
	if c.NArg() > 1 || required && c.NArg() != 1 {
		return "", usageError("supply one text argument or --file")
	}
	text := c.Args().First()
	if required && strings.TrimSpace(text) == "" {
		return "", usageError("input cannot be empty")
	}
	return text, nil
}
