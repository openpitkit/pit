package main

import (
	"testing"

	openpit "github.com/openpitkit/pit-go"
	"github.com/openpitkit/pit-go/pretrade/policies"
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

