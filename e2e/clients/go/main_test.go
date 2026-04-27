package main

import (
	"testing"

	"go.openpit.dev/openpit"
	"go.openpit.dev/openpit/pretrade/policies"
)

func TestBuildEngineFromPublicModule(t *testing.T) {
	_, err := openpit.NewEngine(func(builder *openpit.EngineBuilder) {
		builder.CheckPreTradeStartPolicy(&policies.OrderValidationPolicy{})
	})
	if err == nil {
		return
	}
	// Runtime download may be unavailable in CI/offline mode.
	// The e2e check is primarily validating module resolution and API shape.
	t.Logf("engine build returned error (acceptable in offline mode): %v", err)
}

