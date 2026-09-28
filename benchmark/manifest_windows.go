//go:build windows

package main

import "errors"

func freezeManifest(root string) error {
	return errors.New("freeze-manifest is the macOS path; the Windows manifest is materialize/frozen only")
}
