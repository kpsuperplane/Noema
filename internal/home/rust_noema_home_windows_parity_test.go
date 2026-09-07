//go:build windows

package home

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"golang.org/x/sys/windows"
)

// Rust source: crates/noema-home/src/private_files.rs:283::windows_acl_grants_distinguish_files_and_directories
func TestRustHome_windows_acl_grants_distinguish_files_and_directories(t *testing.T) {
	directory := filepath.Join(t.TempDir(), "private")
	if err := os.Mkdir(directory, 0o700); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(directory, "secret")
	if err := os.WriteFile(path, []byte("secret"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := protectDirectory(directory); err != nil {
		t.Fatalf("protect directory: %v", err)
	}
	if err := ProtectFile(path); err != nil {
		t.Fatalf("protect file: %v", err)
	}

	directoryDescriptor, err := windows.GetNamedSecurityInfo(
		directory,
		windows.SE_FILE_OBJECT,
		windows.DACL_SECURITY_INFORMATION|windows.PROTECTED_DACL_SECURITY_INFORMATION,
	)
	if err != nil {
		t.Fatal(err)
	}
	fileDescriptor, err := windows.GetNamedSecurityInfo(
		path,
		windows.SE_FILE_OBJECT,
		windows.DACL_SECURITY_INFORMATION|windows.PROTECTED_DACL_SECURITY_INFORMATION,
	)
	if err != nil {
		t.Fatal(err)
	}

	// Go uses SID-based ACLs. OI/CI in the directory descriptor is the
	// semantic equivalent of Rust's (OI)(CI) directory grant; files omit it.
	directoryACL := directoryDescriptor.String()
	fileACL := fileDescriptor.String()
	if !strings.Contains(directoryACL, "OICI") {
		t.Fatalf("directory ACL lacks object/container inheritance: %q", directoryACL)
	}
	if strings.Contains(fileACL, "OICI") {
		t.Fatalf("file ACL unexpectedly inherits to children: %q", fileACL)
	}
}
