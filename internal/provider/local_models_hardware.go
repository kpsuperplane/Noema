package provider

import (
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"strconv"
	"strings"
	"time"
)

const (
	localProbeLimit   = 64 << 10
	localProbeTimeout = 2 * time.Second
)

var errLocalProbeTooLarge = errors.New("local hardware probe output is too large")

type limitedProbeOutput struct {
	buffer   bytes.Buffer
	overflow bool
}

func (w *limitedProbeOutput) Write(data []byte) (int, error) {
	length := len(data)
	remaining := localProbeLimit - w.buffer.Len()
	if remaining > 0 {
		if remaining > length {
			remaining = length
		}
		_, _ = w.buffer.Write(data[:remaining])
	}
	if remaining < length {
		w.overflow = true
	}
	return length, nil
}

func localProbeOutput(ctx context.Context, program string, arguments ...string) (string, error) {
	probeContext, cancel := context.WithTimeout(ctx, localProbeTimeout)
	defer cancel()
	var output limitedProbeOutput
	command := exec.CommandContext(probeContext, program, arguments...)
	command.Stdout, command.Stderr = &output, io.Discard
	if err := command.Run(); err != nil {
		if probeContext.Err() != nil {
			return "", probeContext.Err()
		}
		return "", fmt.Errorf("run local hardware probe: %w", err)
	}
	if output.overflow {
		return "", errLocalProbeTooLarge
	}
	return output.buffer.String(), nil
}

func localProbeFile(path string) (string, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", err
	}
	defer file.Close()
	data, err := io.ReadAll(io.LimitReader(file, localProbeLimit+1))
	if err != nil {
		return "", err
	}
	if len(data) > localProbeLimit {
		return "", errLocalProbeTooLarge
	}
	return string(data), nil
}

func parseLinuxRAMGB(value string) (int, bool) {
	for _, line := range strings.Split(value, "\n") {
		fields := strings.Fields(line)
		if len(fields) != 3 || fields[0] != "MemTotal:" || fields[2] != "kB" {
			continue
		}
		kib, err := strconv.ParseUint(fields[1], 10, 64)
		if err != nil {
			return 0, false
		}
		return wholeGiB(kib, 1024*1024)
	}
	return 0, false
}

func parseBytesRAMGB(value string) (int, bool) {
	bytes, err := strconv.ParseUint(strings.TrimSpace(value), 10, 64)
	if err != nil {
		return 0, false
	}
	return wholeGiB(bytes, 1024*1024*1024)
}

func parseMacVRAMGB(value string) *int {
	for _, line := range strings.Split(value, "\n") {
		if !strings.Contains(line, "VRAM") {
			continue
		}
		fields := strings.Fields(line)
		for index := 0; index+1 < len(fields); index++ {
			amount, err := strconv.Atoi(fields[index])
			if err != nil {
				continue
			}
			switch strings.TrimRight(fields[index+1], ",") {
			case "GB":
				return intPointer(amount)
			case "MB":
				if gib, ok := wholeGiB(uint64(amount), 1024); ok {
					return intPointer(gib)
				}
			}
		}
	}
	return nil
}

func wholeGiB(value uint64, units uint64) (int, bool) {
	result := value / units
	if result == 0 || result > uint64(^uint(0)>>1) {
		return 0, false
	}
	return int(result), true
}
