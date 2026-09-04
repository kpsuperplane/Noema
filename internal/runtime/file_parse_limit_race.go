//go:build (linux || darwin) && race

package runtime

// Race instrumentation exceeds the production worker address-space limit.
func limitFileParseWorkerMemory(int) bool {
	return true
}
