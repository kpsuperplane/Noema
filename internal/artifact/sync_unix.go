//go:build !windows

package artifact

import "os"

func syncDirectory(directory *os.Root) error {
	file, err := directory.Open(".")
	if err != nil {
		return err
	}
	defer file.Close()
	return file.Sync()
}
