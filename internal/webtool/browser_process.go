package webtool

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"io"
	"net"
	"net/http"
	"os/exec"
	"strconv"
	"strings"
	"time"

	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
)

const browserFrameLimit = 2 << 20

type browserProcess struct {
	command            *exec.Cmd
	tree               browserProcessTree
	connection         *websocket.Conn
	sessionID, frameID string
	nextID             uint64
	mainDocumentStatus int
}

type obscuraCommandError struct {
	Code   int `json:"code"`
	method string
}

func (e *obscuraCommandError) Error() string {
	return "CDP " + e.method + " failed (" + strconv.Itoa(e.Code) + ")"
}

func startBrowserProcess(ctx context.Context, path string, oldSpaceMB int) (*browserProcess, error) {
	// Upstream reports the configured port, so port zero cannot locate its listener.
	listener, err := net.Listen("tcp4", "127.0.0.1:0")
	if err != nil {
		return nil, err
	}
	address := listener.Addr().String()
	port := strconv.Itoa(listener.Addr().(*net.TCPAddr).Port)
	_ = listener.Close()
	command := exec.Command(path, browserCommandArguments(oldSpaceMB, port)...)
	command.Env = append(browserWorkerEnvironment(), "RUST_LOG=obscura_cdp::server=info")
	configureBrowserCommand(command)
	output, err := command.StderrPipe()
	if err != nil {
		return nil, err
	}
	if err := command.Start(); err != nil {
		return nil, err
	}
	p := &browserProcess{command: command}
	tree, err := attachBrowserCommand(command)
	if err != nil {
		_ = command.Process.Kill()
		_ = command.Wait()
		return nil, err
	}
	p.tree = tree
	ready := make(chan bool, 1)
	go func() {
		scanner := bufio.NewScanner(output)
		total := 0
		for scanner.Scan() {
			total += len(scanner.Bytes())
			if total > browserFrameLimit {
				break
			}
			if strings.Contains(scanner.Text(), "Obscura CDP server listening on ws://"+address) {
				ready <- true
				_, _ = io.Copy(io.Discard, output)
				return
			}
		}
		ready <- false
	}()
	start, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	success := false
	defer func() {
		if !success {
			p.close()
		}
	}()
	select {
	case started := <-ready:
		if !started {
			return nil, errors.New("Obscura did not become ready")
		}
	case <-start.Done():
		return nil, start.Err()
	}
	// The child owns this loopback endpoint. Do not use proxy environment settings.
	p.connection, _, err = websocket.Dial(start, "ws://"+address+"/devtools/browser", &websocket.DialOptions{HTTPClient: &http.Client{Transport: &http.Transport{}}})
	if err != nil {
		return nil, err
	}
	p.connection.SetReadLimit(browserFrameLimit)
	var target struct {
		TargetID string `json:"targetId"`
	}
	if err := p.call(start, "Target.createTarget", map[string]any{"url": "about:blank"}, &target); err != nil {
		return nil, err
	}
	var attached struct {
		SessionID string `json:"sessionId"`
	}
	if err := p.call(start, "Target.attachToTarget", map[string]any{"targetId": target.TargetID, "flatten": true}, &attached); err != nil {
		return nil, err
	}
	if attached.SessionID == "" {
		return nil, errors.New("Obscura session is missing")
	}
	p.sessionID = attached.SessionID
	for _, method := range []string{"Page.enable", "Network.enable"} {
		if err := p.call(start, method, nil, nil); err != nil {
			return nil, err
		}
	}
	var frame struct {
		FrameTree struct {
			Frame struct {
				ID string `json:"id"`
			} `json:"frame"`
		} `json:"frameTree"`
	}
	if err := p.call(start, "Page.getFrameTree", nil, &frame); err != nil {
		return nil, err
	}
	p.frameID = frame.FrameTree.Frame.ID
	if p.frameID == "" {
		return nil, errors.New("Obscura frame is missing")
	}
	success = true
	return p, nil
}

func browserCommandArguments(oldSpaceMB int, port string) []string {
	return []string{"--stealth", "--v8-flags", "--max-old-space-size=" + strconv.Itoa(oldSpaceMB), "serve", "--host", "127.0.0.1", "--port", port, "--max-connections", "1"}
}

// The browser session lock serializes calls and closure.
func (p *browserProcess) call(ctx context.Context, method string, params any, result any) error {
	p.nextID++
	request := map[string]any{"id": p.nextID, "method": method, "params": params}
	if p.sessionID != "" {
		request["sessionId"] = p.sessionID
	}
	if err := wsjson.Write(ctx, p.connection, request); err != nil {
		return err
	}
	for {
		var response struct {
			ID        uint64          `json:"id"`
			SessionID string          `json:"sessionId"`
			Method    string          `json:"method"`
			Params    json.RawMessage `json:"params"`
			Result    json.RawMessage `json:"result"`
			Error     json.RawMessage `json:"error"`
		}
		if err := wsjson.Read(ctx, p.connection, &response); err != nil {
			return err
		}
		if response.SessionID != p.sessionID {
			continue
		}
		if response.Method == "Network.responseReceived" {
			var event struct {
				Type     string `json:"type"`
				FrameID  string `json:"frameId"`
				Response struct {
					Status int `json:"status"`
				} `json:"response"`
			}
			if json.Unmarshal(response.Params, &event) != nil {
				return errors.New("invalid Obscura network event")
			}
			if event.Type == "Document" && event.FrameID == p.frameID {
				p.mainDocumentStatus = event.Response.Status
			}
		}
		if response.ID != p.nextID {
			continue
		}
		if len(response.Error) != 0 {
			failure := &obscuraCommandError{method: method}
			if err := json.Unmarshal(response.Error, failure); err != nil {
				return err
			}
			return failure
		}
		if len(response.Result) == 0 || string(response.Result) == "null" {
			return errors.New("invalid Obscura response")
		}
		if result != nil {
			return json.Unmarshal(response.Result, result)
		}
		return nil
	}
}

func (p *browserProcess) evaluate(ctx context.Context, expression string, target any) error {
	var response struct {
		Result struct {
			Value json.RawMessage `json:"value"`
		} `json:"result"`
		Exception json.RawMessage `json:"exceptionDetails"`
	}
	if err := p.call(ctx, "Runtime.evaluate", map[string]any{"expression": expression, "returnByValue": true, "awaitPromise": true, "timeout": 25000}, &response); err != nil {
		return err
	}
	if len(response.Exception) != 0 {
		return &obscuraCommandError{Code: -32000, method: "Runtime.evaluate"}
	}
	if target != nil {
		return json.Unmarshal(response.Result.Value, target)
	}
	return nil
}

func (p *browserProcess) close() {
	if p.connection != nil {
		_ = p.connection.CloseNow()
		p.connection = nil
	}
	if p.command != nil {
		terminateBrowserCommand(p.tree, p.command)
		_ = p.command.Wait()
		p.tree.close()
		p.command = nil
	}
}
