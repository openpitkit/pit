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

// Package marketdata exposes the Go binding for the OpenPit market-data
// service: a registry of instruments and their latest quotes, shared between
// a feed that publishes quotes and the policies that read them.
package marketdata

import (
	"errors"
	"runtime"
	"runtime/cgo"

	"go.openpit.dev/openpit/internal/callback"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/optional"
)

// Errors returned by Service.Get, mirroring the SDK MarketDataError variants.
var (
	// ErrUnknownInstrument reports that the requested instrument is not
	// registered with the service.
	ErrUnknownInstrument = errors.New("unknown instrument")
	// ErrQuoteUnavailable reports that no usable quote is available for the
	// instrument (never pushed or cleared).
	ErrQuoteUnavailable = errors.New("quote unavailable")
	// ErrQuoteExpired reports that the selected quote aged past its effective
	// TTL. Service.Get returns the stale quote together with this error.
	ErrQuoteExpired = errors.New("quote expired")
)

// Errors returned by the registration methods, mirroring the SDK
// AlreadyRegistered and RegistrationError variants.
var (
	// ErrAlreadyRegistered reports that the instrument is already registered,
	// returned by Register and RegisterWithTTL.
	ErrAlreadyRegistered = errors.New("instrument is already registered")
	// ErrDuplicateID reports that the caller-supplied id is already registered,
	// returned by RegisterWithID and RegisterWithIDAndTTL.
	ErrDuplicateID = errors.New("instrument id is already registered")
	// ErrDuplicateInstrument reports that the instrument is already registered
	// under a different id, returned by RegisterWithID and
	// RegisterWithIDAndTTL.
	ErrDuplicateInstrument = errors.New("instrument is already registered under a different id")
)

// ErrNoTarget is returned by PushFor and PushForPatch when both the account and
// account-group slices are empty (no target was specified).
var ErrNoTarget = errors.New("no target accounts or groups specified")

//------------------------------------------------------------------------------
// Service

// Service wraps a native market-data service handle.
//
// A service is a shared, reference-counted registry: Clone hands out an
// additional handle to the same underlying service so that, for example, a feed
// and a policy can operate on identical state.
type Service struct{ handle native.MarketDataService }

func newServiceFromHandle(handle native.MarketDataService) *Service {
	return &Service{handle: handle}
}

// Close releases this market-data service handle. The underlying service stays
// alive while other handles to it exist.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (s *Service) Close() {
	native.DestroyMarketDataService(s.handle)
	s.handle = nil
}

// Clone returns a new handle referring to the same market-data service. The
// returned handle must be released independently with Close.
func (s *Service) Clone() *Service {
	return newServiceFromHandle(native.MarketDataServiceClone(s.handle))
}

// Handle returns the underlying native handle.
func (s *Service) Handle() native.MarketDataService {
	return s.handle
}

// Register registers instrument with the service-wide default TTL and returns
// its auto-assigned id.
func (s *Service) Register(instrument param.Instrument) (InstrumentID, error) {
	status, id, err := native.MarketDataServiceRegister(s.handle, instrument.Handle())
	runtime.KeepAlive(instrument)
	switch status {
	case native.MarketDataRegisterStatusOk:
		return newInstrumentIDFromHandle(id), nil
	case native.MarketDataRegisterStatusAlreadyRegistered:
		return InstrumentID{}, ErrAlreadyRegistered
	default:
		return InstrumentID{}, err
	}
}

// RegisterWithTTL registers instrument with a per-instrument TTL override and
// returns its auto-assigned id.
func (s *Service) RegisterWithTTL(
	instrument param.Instrument,
	ttl QuoteTTL,
) (InstrumentID, error) {
	status, id, err := native.MarketDataServiceRegisterWithTTL(
		s.handle,
		instrument.Handle(),
		ttl.Handle(),
	)
	runtime.KeepAlive(instrument)
	switch status {
	case native.MarketDataRegisterStatusOk:
		return newInstrumentIDFromHandle(id), nil
	case native.MarketDataRegisterStatusAlreadyRegistered:
		return InstrumentID{}, ErrAlreadyRegistered
	default:
		return InstrumentID{}, err
	}
}

// RegisterWithID registers instrument under the caller-supplied id with the
// service-wide default TTL and returns that id.
func (s *Service) RegisterWithID(
	instrument param.Instrument,
	id InstrumentID,
) (InstrumentID, error) {
	status, outID, err := native.MarketDataServiceRegisterWithID(
		s.handle,
		instrument.Handle(),
		id.Handle(),
	)
	runtime.KeepAlive(instrument)
	switch status {
	case native.MarketDataRegisterStatusOk:
		return newInstrumentIDFromHandle(outID), nil
	case native.MarketDataRegisterStatusDuplicateID:
		return InstrumentID{}, ErrDuplicateID
	case native.MarketDataRegisterStatusDuplicateInstrument:
		return InstrumentID{}, ErrDuplicateInstrument
	default:
		return InstrumentID{}, err
	}
}

// RegisterWithIDAndTTL registers instrument under the caller-supplied id with a
// per-instrument TTL override and returns that id.
func (s *Service) RegisterWithIDAndTTL(
	instrument param.Instrument,
	id InstrumentID,
	ttl QuoteTTL,
) (InstrumentID, error) {
	status, outID, err := native.MarketDataServiceRegisterWithIDAndTTL(
		s.handle,
		instrument.Handle(),
		id.Handle(),
		ttl.Handle(),
	)
	runtime.KeepAlive(instrument)
	switch status {
	case native.MarketDataRegisterStatusOk:
		return newInstrumentIDFromHandle(outID), nil
	case native.MarketDataRegisterStatusDuplicateID:
		return InstrumentID{}, ErrDuplicateID
	case native.MarketDataRegisterStatusDuplicateInstrument:
		return InstrumentID{}, ErrDuplicateInstrument
	default:
		return InstrumentID{}, err
	}
}

// PushFor publishes a quote for instrumentID, replacing the stored snapshot for
// each of the specified accounts and groups. At least one account or group must
// be supplied; passing empty slices returns ErrNoTarget. To target the default
// ("everyone-else") bucket, include [param.DefaultAccountGroup] in
// accountGroupIDs.
func (s *Service) PushFor(
	instrumentID InstrumentID,
	quote Quote,
	accountIDs []param.AccountID,
	accountGroupIDs []param.AccountGroupID,
) error {
	nativeAccounts := make([]native.ParamAccountID, len(accountIDs))
	for i, a := range accountIDs {
		nativeAccounts[i] = a.Handle()
	}
	nativeGroups := make([]native.ParamAccountGroupID, len(accountGroupIDs))
	for i, g := range accountGroupIDs {
		nativeGroups[i] = g.Handle()
	}
	status, err := native.MarketDataServicePushFor(
		s.handle,
		instrumentID.Handle(),
		quote.Handle(),
		nativeAccounts,
		nativeGroups,
	)
	switch status {
	case native.MarketDataRegisterStatusOk:
		return nil
	case native.MarketDataRegisterStatusUnknownInstrument:
		return ErrUnknownInstrument
	case native.MarketDataRegisterStatusNoTarget:
		return ErrNoTarget
	default:
		return err
	}
}

// PushForPatch publishes a partial update for instrumentID, merging it into
// the stored snapshot for each of the specified accounts and groups. At least
// one account or group must be supplied; passing empty slices returns
// ErrNoTarget. To target the default ("everyone-else") bucket, include
// [param.DefaultAccountGroup] in accountGroupIDs.
func (s *Service) PushForPatch(
	instrumentID InstrumentID,
	quote Quote,
	accountIDs []param.AccountID,
	accountGroupIDs []param.AccountGroupID,
) error {
	nativeAccounts := make([]native.ParamAccountID, len(accountIDs))
	for i, a := range accountIDs {
		nativeAccounts[i] = a.Handle()
	}
	nativeGroups := make([]native.ParamAccountGroupID, len(accountGroupIDs))
	for i, g := range accountGroupIDs {
		nativeGroups[i] = g.Handle()
	}
	status, err := native.MarketDataServicePushForPatch(
		s.handle,
		instrumentID.Handle(),
		quote.Handle(),
		nativeAccounts,
		nativeGroups,
	)
	switch status {
	case native.MarketDataRegisterStatusOk:
		return nil
	case native.MarketDataRegisterStatusUnknownInstrument:
		return ErrUnknownInstrument
	case native.MarketDataRegisterStatusNoTarget:
		return ErrNoTarget
	default:
		return err
	}
}

// SetInstrumentTTL updates the TTL of an already-registered instrument.
func (s *Service) SetInstrumentTTL(instrumentID InstrumentID, ttl QuoteTTL) error {
	status := native.MarketDataServiceSetInstrumentTTL(
		s.handle,
		instrumentID.Handle(),
		ttl.Handle(),
	)
	if status == native.MarketDataRegisterStatusOk {
		return nil
	}
	return ErrUnknownInstrument
}

// ClearInstrumentTTL removes the per-instrument TTL override for instrumentID,
// reverting to the service-wide default.
func (s *Service) ClearInstrumentTTL(instrumentID InstrumentID) error {
	status := native.MarketDataServiceClearInstrumentTTL(s.handle, instrumentID.Handle())
	if status == native.MarketDataRegisterStatusOk {
		return nil
	}
	return ErrUnknownInstrument
}

// SetAccountTTL sets a service-wide TTL override for all instruments when
// read by accountID.
func (s *Service) SetAccountTTL(accountID param.AccountID, ttl QuoteTTL) {
	native.MarketDataServiceSetAccountTTL(s.handle, accountID.Handle(), ttl.Handle())
}

// ClearAccountTTL removes the per-account TTL override.
func (s *Service) ClearAccountTTL(accountID param.AccountID) {
	native.MarketDataServiceClearAccountTTL(s.handle, accountID.Handle())
}

// SetAccountGroupTTL sets a service-wide TTL override for all instruments when
// read by accountGroupID. Pass [param.DefaultAccountGroup] to target the
// service-level default-group TTL.
func (s *Service) SetAccountGroupTTL(accountGroupID param.AccountGroupID, ttl QuoteTTL) {
	native.MarketDataServiceSetAccountGroupTTL(s.handle, accountGroupID.Handle(), ttl.Handle())
}

// ClearAccountGroupTTL removes the per-group TTL override. Pass
// [param.DefaultAccountGroup] to target the service-level default-group TTL.
func (s *Service) ClearAccountGroupTTL(accountGroupID param.AccountGroupID) {
	native.MarketDataServiceClearAccountGroupTTL(s.handle, accountGroupID.Handle())
}

// SetInstrumentAccountTTL sets a per-instrument, per-account TTL override.
func (s *Service) SetInstrumentAccountTTL(
	instrumentID InstrumentID,
	accountID param.AccountID,
	ttl QuoteTTL,
) error {
	status := native.MarketDataServiceSetInstrumentAccountTTL(
		s.handle,
		instrumentID.Handle(),
		accountID.Handle(),
		ttl.Handle(),
	)
	if status == native.MarketDataRegisterStatusOk {
		return nil
	}
	return ErrUnknownInstrument
}

// ClearInstrumentAccountTTL removes the per-instrument, per-account TTL
// override.
func (s *Service) ClearInstrumentAccountTTL(
	instrumentID InstrumentID,
	accountID param.AccountID,
) error {
	status := native.MarketDataServiceClearInstrumentAccountTTL(
		s.handle,
		instrumentID.Handle(),
		accountID.Handle(),
	)
	if status == native.MarketDataRegisterStatusOk {
		return nil
	}
	return ErrUnknownInstrument
}

// SetInstrumentAccountGroupTTL sets a per-instrument, per-group TTL override.
// Pass [param.DefaultAccountGroup] to target the instrument's default-group
// TTL cell.
func (s *Service) SetInstrumentAccountGroupTTL(
	instrumentID InstrumentID,
	accountGroupID param.AccountGroupID,
	ttl QuoteTTL,
) error {
	status := native.MarketDataServiceSetInstrumentAccountGroupTTL(
		s.handle,
		instrumentID.Handle(),
		accountGroupID.Handle(),
		ttl.Handle(),
	)
	if status == native.MarketDataRegisterStatusOk {
		return nil
	}
	return ErrUnknownInstrument
}

// ClearInstrumentAccountGroupTTL removes the per-instrument, per-group TTL
// override. Pass [param.DefaultAccountGroup] to target the instrument's
// default-group TTL cell.
func (s *Service) ClearInstrumentAccountGroupTTL(
	instrumentID InstrumentID,
	accountGroupID param.AccountGroupID,
) error {
	status := native.MarketDataServiceClearInstrumentAccountGroupTTL(
		s.handle,
		instrumentID.Handle(),
		accountGroupID.Handle(),
	)
	if status == native.MarketDataRegisterStatusOk {
		return nil
	}
	return ErrUnknownInstrument
}

// Clear clears the stored quote for instrumentID. It is a no-op if
// instrumentID is not registered.
func (s *Service) Clear(instrumentID InstrumentID) {
	native.MarketDataServiceClear(s.handle, instrumentID.Handle())
}

// Push publishes a quote for instrumentID, replacing the entire stored snapshot.
func (s *Service) Push(instrumentID InstrumentID, quote Quote) error {
	status, err := native.MarketDataServicePush(s.handle, instrumentID.Handle(), quote.Handle())
	switch status {
	case native.MarketDataRegisterStatusOk:
		return nil
	case native.MarketDataRegisterStatusUnknownInstrument:
		return ErrUnknownInstrument
	default:
		return err
	}
}

// PushPatch publishes a partial update for instrumentID, merging it into the
// stored snapshot.
func (s *Service) PushPatch(instrumentID InstrumentID, quote Quote) error {
	status, err := native.MarketDataServicePushPatch(s.handle, instrumentID.Handle(), quote.Handle())
	switch status {
	case native.MarketDataRegisterStatusOk:
		return nil
	case native.MarketDataRegisterStatusUnknownInstrument:
		return ErrUnknownInstrument
	default:
		return err
	}
}

// PushByInstrument publishes a quote for instrument, replacing the stored
// snapshot, and returns the instrument's id. If instrument is unregistered, a
// named slot is created with the service-default TTL.
func (s *Service) PushByInstrument(
	instrument param.Instrument,
	quote Quote,
) (InstrumentID, error) {
	id, err := native.MarketDataServicePushByInstrument(
		s.handle,
		instrument.Handle(),
		quote.Handle(),
	)
	runtime.KeepAlive(instrument)
	if err != nil {
		return InstrumentID{}, err
	}
	return newInstrumentIDFromHandle(id), nil
}

// PushByInstrumentPatch publishes a partial update for instrument, merging it
// into the stored snapshot, and returns the instrument's id.
func (s *Service) PushByInstrumentPatch(
	instrument param.Instrument,
	quote Quote,
) (InstrumentID, error) {
	id, err := native.MarketDataServicePushByInstrumentPatch(
		s.handle,
		instrument.Handle(),
		quote.Handle(),
	)
	runtime.KeepAlive(instrument)
	if err != nil {
		return InstrumentID{}, err
	}
	return newInstrumentIDFromHandle(id), nil
}

// GetOptional reads the latest quote for instrumentID with account-aware
// resolution.
// accountID is the requesting account; accountInfo supplies the account's group
// on demand - the core invokes it lazily, only when the fallback chain reaches
// the per-group bucket. resolution controls the fallback chain.
//
// The returned option is set only when a usable quote was found. An
// unavailable, expired, or unknown instrument yields optional.None.
func (s *Service) GetOptional(
	instrumentID InstrumentID,
	accountID param.AccountID,
	accountInfo AccountInfo,
	resolution QuoteResolution,
) optional.Option[Quote] {
	accountInfoHandle := cgo.NewHandle(accountInfo)
	status, quote := native.MarketDataServiceGet(
		s.handle,
		instrumentID.Handle(),
		accountID.Handle(),
		accountGroupResolverFnAddr(),
		callback.NewUserDataFromHandle(accountInfoHandle),
		resolution,
	)
	accountInfoHandle.Delete()
	if status != native.MarketDataGetStatusFound {
		return optional.None[Quote]()
	}
	return optional.Some(newQuoteFromHandle(quote))
}

// Get reads the latest quote for instrumentID with account-aware resolution,
// distinguishing read failures: ErrUnknownInstrument when instrumentID is not
// registered, ErrQuoteUnavailable when it is registered but holds no usable
// quote under the given resolution, and ErrQuoteExpired when the selected quote
// aged past TTL.
//
// On ErrQuoteExpired, the returned Quote is the stale quote selected by the
// core service. Other errors return a zero Quote.
func (s *Service) Get(
	instrumentID InstrumentID,
	accountID param.AccountID,
	accountInfo AccountInfo,
	resolution QuoteResolution,
) (Quote, error) {
	accountInfoHandle := cgo.NewHandle(accountInfo)
	status, quote := native.MarketDataServiceGet(
		s.handle,
		instrumentID.Handle(),
		accountID.Handle(),
		accountGroupResolverFnAddr(),
		callback.NewUserDataFromHandle(accountInfoHandle),
		resolution,
	)
	accountInfoHandle.Delete()
	switch status {
	case native.MarketDataGetStatusFound:
		return newQuoteFromHandle(quote), nil
	case native.MarketDataGetStatusUnknownInstrument:
		return Quote{}, ErrUnknownInstrument
	case native.MarketDataGetStatusQuoteExpired:
		return newQuoteFromHandle(quote), ErrQuoteExpired
	default:
		return Quote{}, ErrQuoteUnavailable
	}
}

// Resolve resolves instrument to its registered id. The boolean result is true
// only when the instrument is registered by name.
func (s *Service) Resolve(instrument param.Instrument) (InstrumentID, bool) {
	id, ok := native.MarketDataServiceResolve(s.handle, instrument.Handle())
	runtime.KeepAlive(instrument)
	if !ok {
		return InstrumentID{}, false
	}
	return newInstrumentIDFromHandle(id), true
}

//------------------------------------------------------------------------------
// Builder

// Builder builds a market-data Service. Obtain one via
// [go.openpit.dev/openpit.SyncedEngineBuilder.MarketData].
type Builder struct {
	defaultTTL QuoteTTL
}

// NewBuilder constructs a Builder from a default TTL.
func NewBuilder(ttl QuoteTTL) *Builder {
	return &Builder{defaultTTL: ttl}
}

// NoSync creates a builder that does not synchronize with the engine.
func (b *Builder) NoSync() *ReadyBuilder {
	return newReadyBuilder(native.SyncPolicyNone, b.defaultTTL)
}

// FullSync creates a builder to use full synchronization, making the
// resulting service safe for concurrent access from multiple goroutines.
func (b *Builder) FullSync() *ReadyBuilder {
	return newReadyBuilder(native.SyncPolicyFull, b.defaultTTL)
}

// ReadyBuilder builds a market-data Service.
type ReadyBuilder struct {
	syncPolicy native.SyncPolicy
	defaultTTL QuoteTTL
}

func newReadyBuilder(syncPolicy native.SyncPolicy, defaultTTL QuoteTTL) *ReadyBuilder {
	return &ReadyBuilder{syncPolicy: syncPolicy, defaultTTL: defaultTTL}
}

// NoSync upgrades the builder to not synchronize with the engine.
func (b *ReadyBuilder) NoSync() *ReadyBuilder {
	return newReadyBuilder(native.SyncPolicyNone, b.defaultTTL)
}

// FullSync upgrades the builder to use full synchronization, making the
// resulting service safe for concurrent access from multiple goroutines.
func (b *ReadyBuilder) FullSync() *ReadyBuilder {
	return newReadyBuilder(native.SyncPolicyFull, b.defaultTTL)
}

// Build constructs the market-data service. On success, ownership of the
// returned service passes to the caller, who must release it by calling Close.
func (b *ReadyBuilder) Build() (*Service, error) {
	svc, err := native.CreateMarketDataService(b.syncPolicy, b.defaultTTL.Handle())
	if err != nil {
		return nil, err
	}
	return newServiceFromHandle(svc), nil
}
