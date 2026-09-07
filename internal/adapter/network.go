package adapter

import (
	"errors"
	"net"
)

var errResolvedTargetUnavailable = errors.New("adapter target is unavailable")

// validateResolvedAddresses is the deterministic post-DNS boundary used by
// adapter tests and by the network client before a hostname is contacted.
func validateResolvedAddresses(addresses []net.IP) error {
	if len(addresses) == 0 {
		return errResolvedTargetUnavailable
	}
	for _, address := range addresses {
		if address == nil || !isPublicResolvedIP(address) {
			return errResolvedTargetUnavailable
		}
	}
	return nil
}

func isPublicResolvedIP(address net.IP) bool {
	address = address.To16()
	if address == nil {
		return false
	}
	return !address.IsUnspecified() && !address.IsLoopback() && !address.IsPrivate() && !address.IsLinkLocalUnicast() && !address.IsLinkLocalMulticast() && !address.IsMulticast()
}
