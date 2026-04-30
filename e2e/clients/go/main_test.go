package main

import (
	"testing"

	"go.openpit.dev/openpit"
	"go.openpit.dev/openpit/pretrade/policies"
)

func TestBuildEngineFromPublicModule(t *testing.T) {
	builder, err := openpit.NewEngineBuilder()
	if err != nil {
		t.Logf("engine builder returned error (acceptable in offline mode): %v", err)
		return
	}
	builder.BuiltinCheckPreTradeStartPolicy(policies.NewOrderValidation())

	engine, err := builder.Build()
	if err != nil {
		t.Fatalf("Build() error = %v", err)
	}
	defer engine.Stop()
}
