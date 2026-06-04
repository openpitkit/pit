// Copyright The Pit Project Owners. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Please see https://github.com/openpitkit and the OWNERS file for details.

package openpit

import (
	"fmt"

	"go.openpit.dev/openpit/asyncengine"
	"go.openpit.dev/openpit/internal/custompolicy"
	"go.openpit.dev/openpit/internal/loader"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/marketdata"
	"go.openpit.dev/openpit/pretrade"
)

//------------------------------------------------------------------------------
// EngineBuilder

// Version is the SDK release version. It must match the runtime library
// version reported by the loaded native runtime; the compatibility check runs
// during initialization of the internal/native package.
const Version = loader.SDKVersion

// EngineBuilder is the initial stage of the engine builder. It only exposes
// sync-policy selection methods. Call FullSync, NoSync, or AccountSync to
// advance to a synced builder where policies can be registered.
type EngineBuilder struct{}

// NewEngineBuilder returns a new engine builder.
// Call FullSync, NoSync, or AccountSync to obtain a synced builder on
// which policies can be registered.
func NewEngineBuilder() *EngineBuilder {
	return &EngineBuilder{}
}

// FullSync configures full thread-safety synchronization and returns a
// SyncedEngineBuilder ready to accept policies. The resulting engine handle is
// safe for concurrent invocation from multiple goroutines as well as sequential
// cross-thread access. Use this when the engine is shared across multiple
// goroutines or when goroutine migration patterns make sequential thread
// pinning impractical.
func (*EngineBuilder) FullSync() *SyncedEngineBuilder {
	return &SyncedEngineBuilder{syncPolicy: native.SyncPolicyFull}
}

// NoSync configures single-thread synchronization and returns a
// SyncedEngineBuilder ready to accept policies. The resulting engine handle
// must stay on the OS thread that created it; calls from any other OS thread
// are undefined behavior. Use this for single-threaded embeddings where
// synchronization overhead must be zero.
func (*EngineBuilder) NoSync() *SyncedEngineBuilder {
	return &SyncedEngineBuilder{syncPolicy: native.SyncPolicyNone}
}

// AccountSync configures account-sharded synchronization and returns an
// AccountSyncedEngineBuilder ready to accept policies. The resulting engine
// handle is safe for concurrent invocation when the caller pins each account
// to a single processing chain (one queue or one worker at a time), so calls
// for the same account are never concurrent.
//
// The AccountSync chain exposes an extra terminal method BuildAsync that
// wraps the engine into an asyncengine.AsyncEngine, which serializes calls
// per account internally.
func (*EngineBuilder) AccountSync() *AccountSyncedEngineBuilder {
	return &AccountSyncedEngineBuilder{
		SyncedEngineBuilder: SyncedEngineBuilder{
			syncPolicy: native.SyncPolicyAccount,
		},
	}
}

//------------------------------------------------------------------------------
// SyncedEngineBuilder

// SyncedEngineBuilder is the second stage of the engine builder chain,
// returned by EngineBuilder.FullSync or NoSync. Add at least one policy to
// advance to ReadyEngineBuilder where Build is available.
type SyncedEngineBuilder struct {
	syncPolicy native.SyncPolicy
}

// MarketData creates a market-data builder whose sync mode is derived from
// the engine sync policy. The resulting [marketdata.Builder] must be built
// before the engine itself.
//
// Call [marketdata.Builder.FullSync] afterwards to upgrade a no-sync builder
// to Full if the resulting service must be accessed concurrently.
func (b *SyncedEngineBuilder) MarketData(defaultTTL marketdata.QuoteTTL) *marketdata.ReadyBuilder {
	result := marketdata.NewBuilder(defaultTTL)
	if b.syncPolicy == native.SyncPolicyNone {
		return result.NoSync()
	}
	return result.FullSync()
}

// PreTrade registers pre-trade policies and advances the builder to
// ReadyEngineBuilder.
func (b *SyncedEngineBuilder) PreTrade(policy ...pretrade.Policy) *ReadyEngineBuilder {
	rb := newReadyEngineBuilder(b)
	for _, p := range policy {
		rb.addPreTradePolicy(p)
	}
	return rb
}

// Builtin registers a built-in entity on the builder.
func (b *SyncedEngineBuilder) Builtin(builtinReadyBuilder builtinReadyBuilder) *ReadyEngineBuilder {
	return newReadyEngineBuilder(b).Builtin(builtinReadyBuilder)
}

//------------------------------------------------------------------------------
// AccountSyncedEngineBuilder

// AccountSyncedEngineBuilder is the AccountSync variant of the synced
// builder. It mirrors SyncedEngineBuilder but advances to
// AccountSyncReadyEngineBuilder so the chain retains access to BuildAsync.
type AccountSyncedEngineBuilder struct {
	SyncedEngineBuilder
}

// PreTrade registers pre-trade policies and advances to
// AccountSyncReadyEngineBuilder.
func (b *AccountSyncedEngineBuilder) PreTrade(
	policy ...pretrade.Policy,
) *AccountSyncReadyEngineBuilder {
	return &AccountSyncReadyEngineBuilder{
		ReadyEngineBuilder: b.SyncedEngineBuilder.PreTrade(policy...),
	}
}

// Builtin registers a built-in entity on the builder and advances to
// AccountSyncReadyEngineBuilder.
func (b *AccountSyncedEngineBuilder) Builtin(
	builtinReadyBuilder builtinReadyBuilder,
) *AccountSyncReadyEngineBuilder {
	return &AccountSyncReadyEngineBuilder{
		ReadyEngineBuilder: b.SyncedEngineBuilder.Builtin(builtinReadyBuilder),
	}
}

//------------------------------------------------------------------------------
// ReadyEngineBuilder

// ReadyEngineBuilder is the third stage of the engine builder chain, obtained
// by calling a policy-add method on SyncedEngineBuilder. Accepts additional
// policies, and builds the engine via Build.
type ReadyEngineBuilder struct {
	handle     native.EngineBuilder
	err        error
	unfinished []interface{ Close() }
}

func newReadyEngineBuilder(sb *SyncedEngineBuilder) *ReadyEngineBuilder {
	handle, err := native.CreateEngineBuilder(sb.syncPolicy)
	if err != nil {
		return &ReadyEngineBuilder{err: err}
	}
	return &ReadyEngineBuilder{handle: handle}
}

// Close releases the builder and any policies that were handed to it but
// never transferred to the engine. Safe to call more than once and safe to
// call after Build; subsequent calls are no-ops.
func (b *ReadyEngineBuilder) Close() {
	{
		for _, entity := range b.unfinished {
			entity.Close()
		}
		b.unfinished = nil
	}
	if b.handle != nil {
		native.DestroyEngineBuilder(b.handle)
		b.handle = nil
	}
}

// Build constructs the engine and releases the builder. The builder is
// closed on both success and failure, so an explicit Close afterwards is a
// no-op. On failure, any policies that were accepted by the builder but not
// transferred to the engine are closed by the builder. On success, ownership
// of the returned engine passes to the caller, who must release it by
// calling Stop. Behavior is undefined if Build is called more than once on
// the same builder.
func (b *ReadyEngineBuilder) Build() (*Engine, error) {
	defer b.Close()

	if b.err != nil {
		return nil, b.err
	}

	handle, buildErr, err := native.EngineBuilderBuild(b.handle)
	if buildErr != nil {
		structured := newEngineBuildErrorFromHandle(buildErr)
		native.DestroyEngineBuildError(buildErr)
		return nil, structured
	}
	if err != nil {
		return nil, err
	}
	return newEngineFromHandle(handle), nil
}

// EngineBuildErrorCode classifies a domain engine-build failure.
type EngineBuildErrorCode = native.EngineBuildErrorCode

// Engine-build failure categories.
const (
	// EngineBuildErrorDuplicatePolicyName reports that two policies were
	// registered under the same name.
	EngineBuildErrorDuplicatePolicyName EngineBuildErrorCode = native.EngineBuildErrorCodeDuplicatePolicyName
	// EngineBuildErrorDuplicatePolicyGroupID reports that two policies were
	// registered under the same policy group id.
	EngineBuildErrorDuplicatePolicyGroupID EngineBuildErrorCode = native.EngineBuildErrorCodeDuplicatePolicyGroupID
	// EngineBuildErrorOther reports an unspecified domain build failure.
	EngineBuildErrorOther EngineBuildErrorCode = native.EngineBuildErrorCodeOther
)

// EngineBuildError is a structured domain error returned by Build when engine
// construction fails its configuration validation. Boundary failures are
// surfaced as plain errors instead.
type EngineBuildError struct {
	// Code is the machine-readable failure category.
	Code EngineBuildErrorCode
	// PolicyName is the offending policy name; set only when Code is
	// EngineBuildErrorDuplicatePolicyName.
	PolicyName string
	// PolicyGroupID is the offending policy group id; set only when Code is
	// EngineBuildErrorDuplicatePolicyGroupID.
	PolicyGroupID uint16
}

func newEngineBuildErrorFromHandle(handle native.EngineBuildError) *EngineBuildError {
	return &EngineBuildError{
		Code:          native.EngineBuildErrorGetCode(handle),
		PolicyName:    native.EngineBuildErrorGetPolicyName(handle),
		PolicyGroupID: native.EngineBuildErrorGetPolicyGroupID(handle),
	}
}

func (e *EngineBuildError) Error() string {
	switch e.Code {
	case EngineBuildErrorDuplicatePolicyName:
		return fmt.Sprintf("engine build failed: duplicate policy name %q", e.PolicyName)
	case EngineBuildErrorDuplicatePolicyGroupID:
		return fmt.Sprintf("engine build failed: duplicate policy group id %d", e.PolicyGroupID)
	case EngineBuildErrorOther:
		return "engine build failed"
	default:
		return fmt.Sprintf("engine build failed: unknown error code %d", e.Code)
	}
}

// PreTrade appends additional pre-trade policies to an already-ready builder.
func (b *ReadyEngineBuilder) PreTrade(policy ...pretrade.Policy) *ReadyEngineBuilder {
	for _, p := range policy {
		// Every policy must go through addPolicy even after a previous
		// failure so that the builder takes responsibility for releasing
		// it.
		b.addPreTradePolicy(p)
	}
	return b
}

// Builtin registers a built-in entity on the builder.
func (b *ReadyEngineBuilder) Builtin(builtinReadyBuilder builtinReadyBuilder) *ReadyEngineBuilder {
	if b.err != nil {
		return b
	}
	if err := builtinReadyBuilder.Build(b.handle); err != nil {
		b.err = err
	}
	return b
}

func (b *ReadyEngineBuilder) addPreTradePolicy(policy pretrade.Policy) {
	scheduleClose := func() { b.unfinished = append(b.unfinished, policy) }

	if b.err != nil {
		scheduleClose()
		return
	}

	handle, err := custompolicy.StartPreTrade(policy)
	if err != nil {
		b.err = newEngineBuilderPolicyAddError(err, policy.Name())
		scheduleClose()
		return
	}
	// The caller-owned reference must always be released. On success, the
	// engine keeps its own reference and will drive the eventual destruction
	// on Stop. On failure, dropping this last reference destroys the policy
	// immediately and, for custom policies, triggers free_user_data, which
	// in turn closes the user-provided implementation.
	defer native.DestroyPretradePreTradePolicy(handle)

	if err := native.EngineBuilderAddPreTradePolicy(
		b.handle, handle,
	); err != nil {
		// No scheduleClose is needed here: the deferred release above
		// drops the last reference to the policy and the native Drop path
		// takes care of closing the user implementation via
		// free_user_data.
		b.err = newEngineBuilderPolicyAddError(err, policy.Name())
	}
}

//------------------------------------------------------------------------------
// AccountSyncReadyEngineBuilder

// AccountSyncReadyEngineBuilder is the AccountSync-specialized ready
// builder. In addition to the methods of ReadyEngineBuilder it exposes
// BuildAsync, which wraps the engine into an asyncengine.AsyncEngine.
type AccountSyncReadyEngineBuilder struct {
	*ReadyEngineBuilder
}

// PreTrade appends additional pre-trade policies and preserves the
// AccountSync return type so the chain can still reach BuildAsync.
func (b *AccountSyncReadyEngineBuilder) PreTrade(
	policy ...pretrade.Policy,
) *AccountSyncReadyEngineBuilder {
	b.ReadyEngineBuilder.PreTrade(policy...)
	return b
}

// Builtin registers a built-in entity and preserves the AccountSync
// return type so the chain can still reach BuildAsync.
func (b *AccountSyncReadyEngineBuilder) Builtin(
	builtinReadyBuilder builtinReadyBuilder,
) *AccountSyncReadyEngineBuilder {
	b.ReadyEngineBuilder.Builtin(builtinReadyBuilder)
	return b
}

// BuildAsync builds the engine and returns an asyncengine.Builder bound
// to it. The async builder owns the underlying engine's lifecycle: its
// StopGraceful and StopHard release the engine after the dispatcher
// workers exit, so callers must not call engine.Stop separately.
//
// Use Build instead when manual per-account dispatch (sharded channels,
// actor model, third-party libraries) is preferred over the bundled
// asyncengine helper.
func (b *AccountSyncReadyEngineBuilder) BuildAsync() (*asyncengine.Builder, error) {
	engine, err := b.Build()
	if err != nil {
		return nil, err
	}
	return asyncengine.NewBuilder(engine).WithStopUnderlying(
		engine.Stop,
	), nil
}

//------------------------------------------------------------------------------
// builder helpers

type engineBuilderPolicyAddError struct {
	err        error
	policyName string
}

func newEngineBuilderPolicyAddError(err error, policyName string) engineBuilderPolicyAddError {
	return engineBuilderPolicyAddError{err: err, policyName: policyName}
}

func (e engineBuilderPolicyAddError) Error() string {
	return fmt.Sprintf("failed to add policy %q: %v", e.policyName, e.err)
}

type builtinReadyBuilder interface {
	Build(native.EngineBuilder) error
}
