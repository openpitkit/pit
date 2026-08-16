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
// Please see https://openpit.dev and the OWNERS file for details.

package asyncengine

import (
	"context"
	"errors"
	"runtime"
	"strings"
	"testing"
	"time"

	"go.openpit.dev/openpit/accountadjustment"
	"go.openpit.dev/openpit/accounts"
	"go.openpit.dev/openpit/configure"
	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/pretrade/policies"
	"go.openpit.dev/openpit/reject"
)

const administrativeChainAccountID uint64 = 7001

var errAdministrativeChainDriverCall = errors.New(
	"unexpected non-administrative driver call",
)

type administrativeChainDriver struct {
	engine native.Engine
}

func (*administrativeChainDriver) StartPreTrade(
	model.Order,
) (*pretrade.Request, []reject.Reject, error) {
	return nil, nil, errAdministrativeChainDriverCall
}

func (*administrativeChainDriver) ExecutePreTrade(
	model.Order,
) (*pretrade.Reservation, []reject.Reject, error) {
	return nil, nil, errAdministrativeChainDriverCall
}

func (*administrativeChainDriver) ExecutePreTradeDryRun(
	model.Order,
) (*pretrade.DryRunReport, error) {
	return nil, errAdministrativeChainDriverCall
}

func (*administrativeChainDriver) ApplyDropCopy(
	model.Order,
) (*pretrade.DropCopyOperation, []reject.Reject, error) {
	return nil, nil, errAdministrativeChainDriverCall
}

func (*administrativeChainDriver) ApplyExecutionReport(
	model.ExecutionReport,
) (pretrade.PostTradeResult, error) {
	return pretrade.PostTradeResult{}, errAdministrativeChainDriverCall
}

func (*administrativeChainDriver) ApplyAccountAdjustment(
	param.AccountID,
	[]model.AccountAdjustment,
) (accountadjustment.BatchResult, error) {
	return accountadjustment.BatchResult{}, errAdministrativeChainDriverCall
}

func (d *administrativeChainDriver) Accounts() accounts.Accounts {
	return accounts.NewFromHandle(d.engine)
}

func (d *administrativeChainDriver) Configure() configure.Configurator {
	return configure.NewFromHandle(d.engine)
}

type administrativeChainState struct {
	accountIDs   []param.AccountID
	otherAccount param.AccountID
	assignment   SpotFundsAccountPnlAssignment
	blocks       []reject.AccountBlock
	currency     param.Asset
	inputErr     error
	hookErr      error
	entered      chan<- struct{}
	release      <-chan struct{}
	resultSeen   bool
	finalSeen    bool
	final        ChainOutcome
}

func (s *administrativeChainState) inputReason(
	context.Context,
) (string, error) {
	if s.inputErr != nil {
		return "", s.inputErr
	}
	return "operator maintenance", nil
}

func (s *administrativeChainState) inputCurrency(
	context.Context,
) (param.Asset, error) {
	if s.inputErr != nil {
		return param.Asset{}, s.inputErr
	}
	return s.currency, nil
}

func (s *administrativeChainState) inputAssignment(
	context.Context,
) (SpotFundsAccountPnlAssignment, error) {
	if s.inputErr != nil {
		return SpotFundsAccountPnlAssignment{}, s.inputErr
	}
	return s.assignment, nil
}

func (s *administrativeChainState) completeResult() error {
	s.resultSeen = true
	if s.entered != nil {
		s.entered <- struct{}{}
	}
	if s.release != nil {
		<-s.release
	}
	return s.hookErr
}

type administrativeChainCase struct {
	name        string
	groupSource bool
	accountLane bool
	hasInput    bool
	newNative   func(*testing.T) native.Engine
	setup       func(*testing.T, *administrativeChainDriver, *administrativeChainState)
	add         func(*ChainBuilder[*administrativeChainState])
	verify      func(*testing.T, *administrativeChainDriver, *administrativeChainState)
}

func administrativeChainCases() []administrativeChainCase {
	return []administrativeChainCase{
		{
			name:     "block account",
			hasInput: true,
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.BlockAccount(BlockHooks[*administrativeChainState]{
					Reason: func(ctx context.Context, state *administrativeChainState) (
						string,
						error,
					) {
						return state.inputReason(ctx)
					},
					OnBlocked: func(
						_ context.Context,
						state *administrativeChainState,
					) error {
						return state.completeResult()
					},
				})
			},
			verify: verifyAdministrativeAccountBlocked,
		},
		{
			name: "unblock account",
			setup: func(
				_ *testing.T,
				driver *administrativeChainDriver,
				state *administrativeChainState,
			) {
				driver.Accounts().Block(state.accountIDs[0], "blocked")
				driver.Accounts().Block(state.otherAccount, "sentinel blocked")
			},
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.UnblockAccount(UnblockHooks[*administrativeChainState]{
					OnUnblocked: func(
						_ context.Context,
						state *administrativeChainState,
					) error {
						return state.completeResult()
					},
				})
			},
			verify: verifyAdministrativeAccountUnblocked,
		},
		{
			name:        "register account group",
			groupSource: true,
			accountLane: true,
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.RegisterAccountGroup(
					administrativeChainAccounts(),
					RegisterAccountGroupHooks[*administrativeChainState]{
						OnRegistered: func(
							_ context.Context,
							state *administrativeChainState,
						) error {
							return state.completeResult()
						},
					},
				)
			},
			verify: func(
				t *testing.T,
				driver *administrativeChainDriver,
				state *administrativeChainState,
			) {
				t.Helper()
				group, ok := driver.Accounts().GroupOf(state.accountIDs[0]).Get()
				if !ok || group != administrativeChainGroup() {
					t.Errorf("GroupOf() = (%v, %v), want source group", group, ok)
				}
				if other := driver.Accounts().GroupOf(state.otherAccount); other.IsSet() {
					t.Errorf("other GroupOf() = %v, want empty option", other)
				}
			},
		},
		{
			name:        "unregister account group",
			groupSource: true,
			accountLane: true,
			setup:       registerAdministrativeChainGroups,
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.UnregisterAccountGroup(
					administrativeChainAccounts(),
					UnregisterAccountGroupHooks[*administrativeChainState]{
						OnUnregistered: func(
							_ context.Context,
							state *administrativeChainState,
						) error {
							return state.completeResult()
						},
					},
				)
			},
			verify: func(
				t *testing.T,
				driver *administrativeChainDriver,
				state *administrativeChainState,
			) {
				t.Helper()
				if group := driver.Accounts().GroupOf(state.accountIDs[0]); group.IsSet() {
					t.Errorf("GroupOf() = %v, want empty option", group)
				}
				other, ok := driver.Accounts().GroupOf(state.otherAccount).Get()
				if !ok || other != administrativeChainOtherGroup() {
					t.Errorf("other GroupOf() = (%v, %v), want other group", other, ok)
				}
			},
		},
		{
			name:        "block account group",
			groupSource: true,
			hasInput:    true,
			setup:       registerAdministrativeChainGroups,
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.BlockAccountGroup(BlockHooks[*administrativeChainState]{
					Reason: func(ctx context.Context, state *administrativeChainState) (
						string,
						error,
					) {
						return state.inputReason(ctx)
					},
					OnBlocked: func(
						_ context.Context,
						state *administrativeChainState,
					) error {
						return state.completeResult()
					},
				})
			},
			verify: verifyAdministrativeAccountGroupBlocked,
		},
		{
			name:        "unblock account group",
			groupSource: true,
			setup: func(
				t *testing.T,
				driver *administrativeChainDriver,
				state *administrativeChainState,
			) {
				registerAdministrativeChainGroups(t, driver, state)
				if err := driver.Accounts().BlockGroup(
					administrativeChainGroup(),
					"blocked",
				); err != nil {
					t.Fatalf("BlockGroup() error = %v", err)
				}
				if err := driver.Accounts().BlockGroup(
					administrativeChainOtherGroup(),
					"sentinel blocked",
				); err != nil {
					t.Fatalf("BlockGroup(other) error = %v", err)
				}
			},
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.UnblockAccountGroup(UnblockHooks[*administrativeChainState]{
					OnUnblocked: func(
						_ context.Context,
						state *administrativeChainState,
					) error {
						return state.completeResult()
					},
				})
			},
			verify: verifyAdministrativeAccountGroupUnblocked,
		},
		{
			name:      "set account currency",
			hasInput:  true,
			newNative: newAdministrativeCurrencyChainNativeEngine,
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.SetAccountCurrency(CurrencyHooks[*administrativeChainState]{
					Currency: func(
						ctx context.Context,
						state *administrativeChainState,
					) (param.Asset, error) {
						return state.inputCurrency(ctx)
					},
					OnSet: func(
						_ context.Context,
						state *administrativeChainState,
					) error {
						return state.completeResult()
					},
				})
			},
			verify: verifyAdministrativeAccountCurrencySet,
		},
		{
			name:      "clear account currency",
			newNative: newAdministrativeCurrencyChainNativeEngine,
			setup: func(
				t *testing.T,
				driver *administrativeChainDriver,
				state *administrativeChainState,
			) {
				if err := driver.Accounts().SetCurrency(
					state.accountIDs[0],
					state.currency,
				); err != nil {
					t.Fatalf("SetCurrency() error = %v", err)
				}
				if err := driver.Accounts().SetCurrency(
					state.otherAccount,
					state.currency,
				); err != nil {
					t.Fatalf("SetCurrency(other) error = %v", err)
				}
			},
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.ClearAccountCurrency(
					ClearCurrencyHooks[*administrativeChainState]{
						OnCleared: func(
							_ context.Context,
							state *administrativeChainState,
						) error {
							return state.completeResult()
						},
					},
				)
			},
			verify: verifyAdministrativeAccountCurrencyCleared,
		},
		{
			name:        "set account-group currency",
			groupSource: true,
			hasInput:    true,
			newNative:   newAdministrativeCurrencyChainNativeEngine,
			setup:       registerAdministrativeChainGroups,
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.SetAccountGroupCurrency(
					CurrencyHooks[*administrativeChainState]{
						Currency: func(
							ctx context.Context,
							state *administrativeChainState,
						) (param.Asset, error) {
							return state.inputCurrency(ctx)
						},
						OnSet: func(
							_ context.Context,
							state *administrativeChainState,
						) error {
							return state.completeResult()
						},
					},
				)
			},
			verify: verifyAdministrativeAccountGroupCurrencySet,
		},
		{
			name:        "clear account-group currency",
			groupSource: true,
			newNative:   newAdministrativeCurrencyChainNativeEngine,
			setup: func(
				t *testing.T,
				driver *administrativeChainDriver,
				state *administrativeChainState,
			) {
				registerAdministrativeChainGroups(t, driver, state)
				if err := driver.Accounts().SetGroupCurrency(
					administrativeChainGroup(),
					state.currency,
				); err != nil {
					t.Fatalf("SetGroupCurrency() error = %v", err)
				}
				if err := driver.Accounts().SetGroupCurrency(
					administrativeChainOtherGroup(),
					state.currency,
				); err != nil {
					t.Fatalf("SetGroupCurrency(other) error = %v", err)
				}
			},
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.ClearAccountGroupCurrency(
					ClearCurrencyHooks[*administrativeChainState]{
						OnCleared: func(
							_ context.Context,
							state *administrativeChainState,
						) error {
							return state.completeResult()
						},
					},
				)
			},
			verify: verifyAdministrativeAccountGroupCurrencyCleared,
		},
		{
			name:      "set SpotFunds account P&L",
			hasInput:  true,
			newNative: newAdministrativePnlChainNativeEngine,
			setup: func(
				t *testing.T,
				driver *administrativeChainDriver,
				state *administrativeChainState,
			) {
				if err := driver.Accounts().SetCurrency(
					state.accountIDs[0],
					state.currency,
				); err != nil {
					t.Fatalf("SetCurrency() error = %v", err)
				}
			},
			add: func(builder *ChainBuilder[*administrativeChainState]) {
				builder.SetSpotFundsAccountPnl(
					SpotFundsAccountPnlHooks[*administrativeChainState]{
						Assignment: func(
							ctx context.Context,
							state *administrativeChainState,
						) (SpotFundsAccountPnlAssignment, error) {
							return state.inputAssignment(ctx)
						},
						OnSet: func(
							_ context.Context,
							state *administrativeChainState,
							result configure.PolicyConfigurationResult,
						) error {
							state.blocks = append(state.blocks, result.AccountBlocks...)
							return state.completeResult()
						},
					},
				)
			},
			verify: verifyAdministrativePnlBlocks,
		},
	}
}

func TestAdministrativeChainStepsReturnResultsAndSerializeTheirLane(t *testing.T) {
	for _, test := range administrativeChainCases() {
		t.Run(test.name, func(t *testing.T) {
			state, driver, engine := newAdministrativeChainTest(t, test)
			entered := make(chan struct{}, 1)
			release := make(chan struct{})
			released := false
			defer func() {
				if !released {
					close(release)
				}
			}()
			state.entered = entered
			state.release = release

			chainFuture := runAdministrativeChainCase(test, state, engine)
			select {
			case <-entered:
			case <-time.After(chainStopTimeout):
				t.Fatal("result hook was not reached")
			}

			competingRan := make(chan struct{}, 1)
			var awaitCompeting func() error
			if test.groupSource && !test.accountLane {
				competingFuture := Chain(
					administrativeChainGroup(),
					func(context.Context) (struct{}, error) { return struct{}{}, nil },
				).Then(func(context.Context, struct{}) error {
					competingRan <- struct{}{}
					return nil
				}).Run(context.Background(), engine)
				awaitCompeting = func() error {
					_, err := competingFuture.Await(context.Background())
					return err
				}
			} else {
				account := administrativeChainAccount()
				if test.accountLane {
					account = state.accountIDs[0]
				}
				competingFuture := engine.Submit(context.Background(), account, func() error {
					competingRan <- struct{}{}
					return nil
				})
				awaitCompeting = func() error {
					_, err := competingFuture.Await(context.Background())
					return err
				}
			}
			select {
			case <-competingRan:
				t.Fatal("competing task interleaved with the result hook")
			case <-time.After(negativeProbeTimeout):
			}

			close(release)
			released = true
			outcome, err := chainFuture.Await(context.Background())
			if err != nil {
				t.Fatalf("chain Await() error = %v", err)
			}
			if outcome.Status != ChainOutcomeCompleted || !outcome.RetryUnsafe {
				t.Errorf("outcome = %+v, want completed retry-unsafe", outcome)
			}
			if err := awaitCompeting(); err != nil {
				t.Fatalf("competing Await() error = %v", err)
			}
			if !state.resultSeen {
				t.Error("result hook did not receive the successful operation")
			}
			if !state.finalSeen || state.final != outcome {
				t.Errorf("Finally outcome = %+v, want %+v", state.final, outcome)
			}
			if test.verify != nil {
				test.verify(t, driver, state)
			}
		})
	}
}

func TestAdministrativeChainResultHookFailureIsRetryUnsafe(t *testing.T) {
	errHook := errors.New("persist administrative result")
	for _, test := range administrativeChainCases() {
		t.Run(test.name, func(t *testing.T) {
			state, driver, engine := newAdministrativeChainTest(t, test)
			state.hookErr = errHook
			outcome, err := runAdministrativeChainCase(test, state, engine).
				Await(context.Background())
			if !errors.Is(err, errHook) {
				t.Fatalf("Await() error = %v, want result hook error", err)
			}
			if !errors.Is(err, ErrChainRetryUnsafe) {
				t.Errorf("Await() error = %v, want ErrChainRetryUnsafe", err)
			}
			if outcome.Status != ChainOutcomeFailed || !outcome.RetryUnsafe {
				t.Errorf("outcome = %+v, want failed retry-unsafe", outcome)
			}
			if !state.resultSeen {
				t.Error("result hook was not called")
			}
			if !state.finalSeen || !state.final.RetryUnsafe {
				t.Errorf("Finally outcome = %+v, want retry-unsafe", state.final)
			}
			if test.verify != nil {
				test.verify(t, driver, state)
			}
		})
	}
}

func TestAdministrativeChainInputHookFailureIsRetrySafe(t *testing.T) {
	errInput := errors.New("read administrative input")
	for _, test := range administrativeChainCases() {
		if !test.hasInput {
			continue
		}
		t.Run(test.name, func(t *testing.T) {
			state, _, engine := newAdministrativeChainTest(t, test)
			state.inputErr = errInput
			outcome, err := runAdministrativeChainCase(test, state, engine).
				Await(context.Background())
			if !errors.Is(err, errInput) {
				t.Fatalf("Await() error = %v, want input hook error", err)
			}
			if errors.Is(err, ErrChainRetryUnsafe) {
				t.Errorf("Await() error = %v, unexpectedly retry-unsafe", err)
			}
			if outcome.Status != ChainOutcomeFailed || outcome.RetryUnsafe {
				t.Errorf("outcome = %+v, want failed retry-safe", outcome)
			}
			if state.resultSeen {
				t.Error("result hook ran after input failure")
			}
			if !state.finalSeen || state.final.RetryUnsafe {
				t.Errorf("Finally outcome = %+v, want retry-safe", state.final)
			}
		})
	}
}

func TestAdministrativeChainMembershipRejectsMissingAccounts(t *testing.T) {
	tests := []struct {
		name     string
		accounts []param.AccountID
		add      func(
			*ChainBuilder[*administrativeChainState],
			[]param.AccountID,
			func(context.Context, *administrativeChainState) error,
		)
	}{
		{
			name: "register nil",
			add: func(
				builder *ChainBuilder[*administrativeChainState],
				accounts []param.AccountID,
				onResult func(context.Context, *administrativeChainState) error,
			) {
				builder.RegisterAccountGroup(
					accounts,
					RegisterAccountGroupHooks[*administrativeChainState]{
						OnRegistered: onResult,
					},
				)
			},
		},
		{
			name:     "register empty",
			accounts: []param.AccountID{},
			add: func(
				builder *ChainBuilder[*administrativeChainState],
				accounts []param.AccountID,
				onResult func(context.Context, *administrativeChainState) error,
			) {
				builder.RegisterAccountGroup(
					accounts,
					RegisterAccountGroupHooks[*administrativeChainState]{
						OnRegistered: onResult,
					},
				)
			},
		},
		{
			name: "unregister nil",
			add: func(
				builder *ChainBuilder[*administrativeChainState],
				accounts []param.AccountID,
				onResult func(context.Context, *administrativeChainState) error,
			) {
				builder.UnregisterAccountGroup(
					accounts,
					UnregisterAccountGroupHooks[*administrativeChainState]{
						OnUnregistered: onResult,
					},
				)
			},
		},
		{
			name:     "unregister empty",
			accounts: []param.AccountID{},
			add: func(
				builder *ChainBuilder[*administrativeChainState],
				accounts []param.AccountID,
				onResult func(context.Context, *administrativeChainState) error,
			) {
				builder.UnregisterAccountGroup(
					accounts,
					UnregisterAccountGroupHooks[*administrativeChainState]{
						OnUnregistered: onResult,
					},
				)
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			state := newAdministrativeChainState(t)
			driver := &administrativeChainDriver{engine: newChainNativeEngine(t)}
			engine := newChainEngine(t, driver)
			beginSeen := false
			resultSeen := false
			finallySeen := false
			builder := Chain(
				administrativeChainGroup(),
				func(context.Context) (*administrativeChainState, error) {
					beginSeen = true
					return state, nil
				},
			)
			test.add(
				builder,
				test.accounts,
				func(context.Context, *administrativeChainState) error {
					resultSeen = true
					return nil
				},
			)
			outcome, err := builder.Finally(func(
				context.Context,
				*administrativeChainState,
				ChainOutcome,
			) error {
				finallySeen = true
				return nil
			}).Run(context.Background(), engine).Await(context.Background())
			if !errors.Is(err, ErrMissingAccountID) {
				t.Fatalf("Await() error = %v, want ErrMissingAccountID", err)
			}
			if outcome.Status != ChainOutcomeFailed || outcome.RetryUnsafe {
				t.Errorf("outcome = %+v, want failed retry-safe", outcome)
			}
			if beginSeen || resultSeen || finallySeen {
				t.Errorf(
					"callbacks = (begin %v, result %v, finally %v), want none",
					beginSeen,
					resultSeen,
					finallySeen,
				)
			}
			if group := driver.Accounts().GroupOf(state.accountIDs[0]); group.IsSet() {
				t.Errorf("GroupOf() = %v, want no mutation", group)
			}
		})
	}
}

func TestAdministrativeChainSurfacesDomainAndConfigurationErrors(t *testing.T) {
	t.Run("account-group domain error", func(t *testing.T) {
		chainCase := administrativeChainCaseByName(t, "register account group")
		state, driver, engine := newAdministrativeChainTest(t, chainCase)
		if err := driver.Accounts().RegisterGroup(
			state.accountIDs,
			administrativeChainOtherGroup(),
		); err != nil {
			t.Fatalf("RegisterGroup(other) error = %v", err)
		}
		outcome, err := runAdministrativeChainCase(chainCase, state, engine).
			Await(context.Background())
		var groupErr *reject.AccountGroupError
		if !errors.As(err, &groupErr) {
			t.Fatalf("Await() error = %T %v, want *reject.AccountGroupError", err, err)
		}
		verifyAdministrativeFailedMutation(t, outcome, state)
	})

	t.Run("configuration error", func(t *testing.T) {
		chainCase := administrativeChainCaseByName(t, "set SpotFunds account P&L")
		state, _, engine := newAdministrativeChainTest(t, chainCase)
		state.assignment.PolicyName = "missing-policy"
		outcome, err := runAdministrativeChainCase(chainCase, state, engine).
			Await(context.Background())
		var configErr *configure.Error
		if !errors.As(err, &configErr) {
			t.Fatalf("Await() error = %T %v, want *configure.Error", err, err)
		}
		if configErr.Kind != configure.ErrorKindUnknown {
			t.Errorf("configuration error kind = %v, want unknown", configErr.Kind)
		}
		verifyAdministrativeFailedMutation(t, outcome, state)
	})
}

func verifyAdministrativeFailedMutation(
	t *testing.T,
	outcome ChainOutcome,
	state *administrativeChainState,
) {
	t.Helper()
	if outcome.Status != ChainOutcomeFailed || !outcome.RetryUnsafe {
		t.Errorf("outcome = %+v, want failed retry-unsafe", outcome)
	}
	if state.resultSeen {
		t.Error("result hook ran after driver error")
	}
	if !state.finalSeen || !state.final.RetryUnsafe {
		t.Errorf("Finally outcome = %+v, want retry-unsafe", state.final)
	}
}

func TestChainAccountAndGroupSourcesUseIndependentLanes(t *testing.T) {
	nativeEngine := newChainNativeEngine(t)
	driver := &administrativeChainDriver{engine: nativeEngine}
	engine := newChainEngine(t, driver)
	const sharedID = 31
	group := param.NewAccountGroupIDFromHandle(native.ParamAccountGroupID(sharedID))
	account := param.NewAccountIDFromUint64(sharedID)
	firstEntered := make(chan struct{})
	releaseFirst := make(chan struct{})
	released := false
	defer func() {
		if !released {
			close(releaseFirst)
		}
	}()

	first := Chain(
		group,
		func(context.Context) (struct{}, error) {
			close(firstEntered)
			<-releaseFirst
			return struct{}{}, nil
		},
	).Run(context.Background(), engine)
	<-firstEntered
	second := Chain(
		account,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).Run(context.Background(), engine)
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	if _, err := second.Await(ctx); err != nil {
		t.Fatalf("account Await() error = %v, want an independent lane", err)
	}
	close(releaseFirst)
	released = true
	if _, err := first.Await(context.Background()); err != nil {
		t.Fatalf("group Await() error = %v", err)
	}
}

func TestChainDistinctGroupSourcesUseIndependentLanes(t *testing.T) {
	nativeEngine := newChainNativeEngine(t)
	driver := &administrativeChainDriver{engine: nativeEngine}
	engine := newChainEngine(t, driver)
	firstGroup := param.NewAccountGroupIDFromHandle(native.ParamAccountGroupID(31))
	secondGroup := param.NewAccountGroupIDFromHandle(native.ParamAccountGroupID(32))
	firstEntered := make(chan struct{})
	releaseFirst := make(chan struct{})
	released := false
	defer func() {
		if !released {
			close(releaseFirst)
		}
	}()

	first := Chain(
		firstGroup,
		func(context.Context) (struct{}, error) {
			close(firstEntered)
			<-releaseFirst
			return struct{}{}, nil
		},
	).Run(context.Background(), engine)
	<-firstEntered
	second := Chain(
		secondGroup,
		func(context.Context) (struct{}, error) { return struct{}{}, nil },
	).Run(context.Background(), engine)
	ctx, cancel := context.WithTimeout(context.Background(), time.Second)
	defer cancel()
	if _, err := second.Await(ctx); err != nil {
		t.Fatalf("second group Await() error = %v, want an independent lane", err)
	}
	close(releaseFirst)
	released = true
	if _, err := first.Await(context.Background()); err != nil {
		t.Fatalf("first group Await() error = %v", err)
	}
}

func TestAdministrativeChainSourceValidation(t *testing.T) {
	engine := newChainEngine(t, &administrativeChainDriver{
		engine: newChainNativeEngine(t),
	})
	account := administrativeChainAccount()
	group := administrativeChainGroup()
	begin := func(context.Context) (*administrativeChainState, error) {
		return newAdministrativeChainState(t), nil
	}

	accountOnly := Chain(group, begin).UnblockAccount(
		UnblockHooks[*administrativeChainState]{
			OnUnblocked: func(context.Context, *administrativeChainState) error {
				return nil
			},
		},
	)
	if _, err := accountOnly.Run(context.Background(), engine).
		Await(context.Background()); !errors.Is(err, ErrChainAccountRequired) {
		t.Errorf("account-only step error = %v, want ErrChainAccountRequired", err)
	}

	groupOnly := Chain(account, begin).UnblockAccountGroup(
		UnblockHooks[*administrativeChainState]{
			OnUnblocked: func(context.Context, *administrativeChainState) error {
				return nil
			},
		},
	)
	if _, err := groupOnly.Run(context.Background(), engine).
		Await(context.Background()); !errors.Is(err, ErrChainAccountGroupRequired) {
		t.Errorf(
			"group-only step error = %v, want ErrChainAccountGroupRequired",
			err,
		)
	}

	mixedGroupLanes := Chain(group, begin).RegisterAccountGroup(
		administrativeChainAccounts(),
		RegisterAccountGroupHooks[*administrativeChainState]{
			OnRegistered: func(context.Context, *administrativeChainState) error {
				return nil
			},
		},
	).UnblockAccountGroup(UnblockHooks[*administrativeChainState]{
		OnUnblocked: func(context.Context, *administrativeChainState) error {
			return nil
		},
	})
	if _, err := mixedGroupLanes.Run(context.Background(), engine).
		Await(context.Background()); !errors.Is(err, ErrChainAccountGroupRequired) {
		t.Errorf(
			"mixed group lane error = %v, want ErrChainAccountGroupRequired",
			err,
		)
	}
}

func TestChainRejectsUninitializedAccountGroupID(t *testing.T) {
	strategy := &recordingStrategy{}
	engine := newAsyncEngine(newFakeDriver(), nil, strategy)
	beginCalled := false
	chain := Chain(
		param.AccountGroupID{},
		func(context.Context) (struct{}, error) {
			beginCalled = true
			return struct{}{}, nil
		},
	)
	result := chain.Run(context.Background(), engine)
	if !result.Done() {
		t.Fatal("future is unresolved, want synchronous rejection")
	}
	outcome, err := result.Await(context.Background())
	if !errors.Is(err, ErrUninitializedAccountGroupID) {
		t.Fatalf("Await() error = %v, want ErrUninitializedAccountGroupID", err)
	}
	if !errors.Is(outcome.Err, ErrUninitializedAccountGroupID) {
		t.Errorf("outcome error = %v, want ErrUninitializedAccountGroupID", outcome.Err)
	}
	if beginCalled {
		t.Error("Chain.Begin ran for an uninitialized account-group source")
	}
	if got := strategy.submitCount(); got != 0 {
		t.Errorf("submit count = %d, want 0", got)
	}
}

func TestChainSetSpotFundsAccountPnlReturnsNumericAndHaltedBlocks(t *testing.T) {
	tests := []struct {
		name  string
		state func(*testing.T) model.PnlState
	}{
		{
			name: "numeric breach",
			state: func(t *testing.T) model.PnlState {
				return model.NewPnlState(mustAdministrativePnl(t, "-20"))
			},
		},
		{
			name: "halted",
			state: func(t *testing.T) model.PnlState {
				halted, err := model.NewPnlHaltedState(
					model.PnlHaltReasonMissingFx,
				)
				if err != nil {
					t.Fatalf("NewPnlHaltedState() error = %v", err)
				}
				return halted
			},
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			chainCase := administrativeChainCaseByName(
				t,
				"set SpotFunds account P&L",
			)
			state, driver, engine := newAdministrativeChainTest(t, chainCase)
			state.assignment.State = test.state(t)
			outcome, err := runAdministrativeChainCase(chainCase, state, engine).
				Await(context.Background())
			if err != nil {
				t.Fatalf("Await() error = %v", err)
			}
			if outcome.Status != ChainOutcomeCompleted {
				t.Errorf("outcome = %+v, want completed", outcome)
			}
			verifyAdministrativePnlBlocks(t, driver, state)
		})
	}
}

func administrativeChainCaseByName(
	t *testing.T,
	name string,
) administrativeChainCase {
	t.Helper()
	for _, test := range administrativeChainCases() {
		if test.name == name {
			return test
		}
	}
	t.Fatalf("administrative chain case %q not found", name)
	return administrativeChainCase{}
}

func newAdministrativeChainTest(
	t *testing.T,
	test administrativeChainCase,
) (*administrativeChainState, *administrativeChainDriver, *AsyncEngine) {
	t.Helper()
	state := newAdministrativeChainState(t)
	newNative := test.newNative
	if newNative == nil {
		newNative = newChainNativeEngine
	}
	driver := &administrativeChainDriver{engine: newNative(t)}
	if test.setup != nil {
		test.setup(t, driver, state)
	}
	return state, driver, newChainEngine(t, driver)
}

func newAdministrativeChainState(t *testing.T) *administrativeChainState {
	t.Helper()
	currency, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	return &administrativeChainState{
		accountIDs:   administrativeChainAccounts(),
		otherAccount: administrativeChainOtherAccount(),
		assignment: SpotFundsAccountPnlAssignment{
			PolicyName: policies.SpotFundsPolicyName,
			State:      model.NewPnlState(mustAdministrativePnl(t, "-20")),
		},
		currency: currency,
	}
}

func runAdministrativeChainCase(
	test administrativeChainCase,
	state *administrativeChainState,
	engine *AsyncEngine,
) *future.Future[ChainOutcome] {
	begin := func(context.Context) (*administrativeChainState, error) {
		return state, nil
	}
	var builder *ChainBuilder[*administrativeChainState]
	if test.groupSource {
		builder = Chain(administrativeChainGroup(), begin)
	} else {
		builder = Chain(administrativeChainAccount(), begin)
	}
	test.add(builder)
	return builder.Finally(func(
		_ context.Context,
		state *administrativeChainState,
		outcome ChainOutcome,
	) error {
		state.finalSeen = true
		state.final = outcome
		return nil
	}).Run(context.Background(), engine)
}

func administrativeChainAccount() param.AccountID {
	return param.NewAccountIDFromUint64(administrativeChainAccountID)
}

func administrativeChainAccounts() []param.AccountID {
	return []param.AccountID{administrativeChainAccount()}
}

func administrativeChainOtherAccount() param.AccountID {
	return param.NewAccountIDFromUint64(administrativeChainAccountID + 1)
}

func administrativeChainGroup() param.AccountGroupID {
	return param.NewAccountGroupIDFromHandle(native.ParamAccountGroupID(17))
}

func administrativeChainOtherGroup() param.AccountGroupID {
	return param.NewAccountGroupIDFromHandle(native.ParamAccountGroupID(18))
}

func registerAdministrativeChainGroup(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	t.Helper()
	if err := driver.Accounts().RegisterGroup(
		state.accountIDs,
		administrativeChainGroup(),
	); err != nil {
		t.Fatalf("RegisterGroup() error = %v", err)
	}
}

func registerAdministrativeChainGroups(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	t.Helper()
	registerAdministrativeChainGroup(t, driver, state)
	if err := driver.Accounts().RegisterGroup(
		[]param.AccountID{state.otherAccount},
		administrativeChainOtherGroup(),
	); err != nil {
		t.Fatalf("RegisterGroup(other) error = %v", err)
	}
}

func observeAdministrativePreTrade(
	t *testing.T,
	driver *administrativeChainDriver,
	account param.AccountID,
) (*pretrade.Request, []reject.Reject) {
	t.Helper()
	order := buildChainCheckOrder(t)
	operation := order.EnsureOperationView()
	operation.SetAccountID(account)
	request, startReject, err := native.EngineStartPreTrade(
		driver.engine,
		order.Handle(),
	)
	runtime.KeepAlive(order)
	if err != nil {
		t.Fatalf("EngineStartPreTrade() error = %v", err)
	}
	if startReject == nil {
		return pretrade.NewRequestFromHandle(request), nil
	}
	rejects, err := reject.NewListFromHandle(startReject)
	native.DestroyPretradeRejectList(startReject)
	if err != nil {
		t.Fatalf("NewListFromHandle() error = %v", err)
	}
	return nil, rejects
}

func verifyAdministrativeAccountPasses(
	t *testing.T,
	driver *administrativeChainDriver,
	account param.AccountID,
) {
	t.Helper()
	request, rejects := observeAdministrativePreTrade(t, driver, account)
	if len(rejects) != 0 {
		t.Fatalf("account %v rejects = %v, want none", account, rejects)
	}
	if request == nil {
		t.Fatalf("account %v request is nil, want accepted", account)
	}
	request.Close()
}

func verifyAdministrativeAccountBlock(
	t *testing.T,
	driver *administrativeChainDriver,
	account param.AccountID,
	wantReason string,
) {
	t.Helper()
	request, rejects := observeAdministrativePreTrade(t, driver, account)
	if request != nil {
		request.Close()
		t.Fatalf("account %v request is non-nil, want blocked", account)
	}
	if len(rejects) != 1 {
		t.Fatalf("account %v rejects = %v, want one", account, rejects)
	}
	if rejects[0].Code != reject.CodeAccountBlocked ||
		rejects[0].Reason != wantReason {
		t.Fatalf(
			"account %v reject = %+v, want account block reason %q",
			account,
			rejects[0],
			wantReason,
		)
	}
}

func verifyAdministrativeAccountBlocked(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	t.Helper()
	verifyAdministrativeAccountBlock(
		t, driver, state.accountIDs[0], "operator maintenance",
	)
	verifyAdministrativeAccountPasses(t, driver, state.otherAccount)
}

func verifyAdministrativeAccountUnblocked(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	t.Helper()
	verifyAdministrativeAccountPasses(t, driver, state.accountIDs[0])
	verifyAdministrativeAccountBlock(
		t, driver, state.otherAccount, "sentinel blocked",
	)
}

func verifyAdministrativeAccountGroupBlocked(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	verifyAdministrativeAccountBlocked(t, driver, state)
}

func verifyAdministrativeAccountGroupUnblocked(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	verifyAdministrativeAccountUnblocked(t, driver, state)
}

func verifyAdministrativeCurrencyBlock(
	t *testing.T,
	blocks map[param.AccountID]reject.AccountBlock,
	account param.AccountID,
	wantReason string,
	wantDetails ...string,
) {
	t.Helper()
	block, ok := blocks[account]
	if !ok {
		t.Fatalf("configured blocks = %v, want account %v", blocks, account)
	}
	if block.Code != reject.CodePnlKillSwitchTriggered ||
		block.Policy != policies.SpotFundsPolicyName ||
		block.Reason != wantReason {
		t.Fatalf(
			"account %v block = %+v, want policy %q reason %q",
			account,
			block,
			policies.SpotFundsPolicyName,
			wantReason,
		)
	}
	for _, want := range wantDetails {
		if !strings.Contains(block.Details, want) {
			t.Errorf("block details = %q, want %q", block.Details, want)
		}
	}
}

func verifyAdministrativeCurrencyMutation(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
	targetSet bool,
) {
	t.Helper()
	accounts := []param.AccountID{state.accountIDs[0], state.otherAccount}
	for _, account := range accounts {
		result, err := driver.Configure().SetSpotFundsAccountPnl(
			policies.SpotFundsPolicyName,
			account,
			model.NewPnlState(mustAdministrativePnl(t, "-20")),
		)
		if err != nil {
			t.Fatalf("SetSpotFundsAccountPnl(%v) error = %v", account, err)
		}
		if len(result.AccountBlocks) != 0 {
			t.Fatalf("seed blocks for account %v = %v, want none", account, result.AccountBlocks)
		}
	}
	eur, err := param.NewAsset("EUR")
	if err != nil {
		t.Fatalf("NewAsset(EUR) error = %v", err)
	}
	barrier := policies.SpotFundsPnlBoundsBarrier{
		Currency:   eur,
		LowerBound: optional.Some(mustAdministrativePnl(t, "-10")),
	}
	outcomes, err := driver.Configure().SpotFundsPnlBoundsKillSwitch(
		policies.SpotFundsPolicyName,
		optional.Some(&barrier),
		nil,
		nil,
	)
	if err != nil {
		t.Fatalf("SpotFundsPnlBoundsKillSwitch() error = %v", err)
	}
	if len(outcomes.AccountBlocks) != 1 {
		t.Fatalf("configured blocks = %v, want one", outcomes.AccountBlocks)
	}
	blocks := make(map[param.AccountID]reject.AccountBlock, 1)
	for _, outcome := range outcomes.AccountBlocks {
		blocks[outcome.AccountID] = outcome.Block
	}
	blockedAccount := state.accountIDs[0]
	unblockedAccount := state.otherAccount
	if targetSet {
		blockedAccount, unblockedAccount = unblockedAccount, blockedAccount
	}
	verifyAdministrativeCurrencyBlock(
		t,
		blocks,
		blockedAccount,
		"pnl kill switch triggered",
		"lower bound breached",
		"currency EUR",
	)
	if _, ok := blocks[unblockedAccount]; ok {
		t.Errorf("configured blocks = %v, account %v must remain unblocked", blocks, unblockedAccount)
	}
}

func verifyAdministrativeAccountCurrencySet(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	verifyAdministrativeCurrencyMutation(t, driver, state, true)
}

func verifyAdministrativeAccountCurrencyCleared(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	verifyAdministrativeCurrencyMutation(t, driver, state, false)
}

func verifyAdministrativeAccountGroupCurrencySet(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	verifyAdministrativeCurrencyMutation(t, driver, state, true)
}

func verifyAdministrativeAccountGroupCurrencyCleared(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	verifyAdministrativeCurrencyMutation(t, driver, state, false)
}

func newAdministrativeCurrencyChainNativeEngine(
	t *testing.T,
) native.Engine {
	t.Helper()
	policy := policies.BuildSpotFunds()
	return buildAdministrativePnlChainNativeEngine(t, policy.Build)
}

func newAdministrativePnlChainNativeEngine(t *testing.T) native.Engine {
	t.Helper()
	usd, err := param.NewAsset("USD")
	if err != nil {
		t.Fatalf("NewAsset(USD) error = %v", err)
	}
	barrier := policies.SpotFundsPnlBoundsBarrier{
		Currency:   usd,
		LowerBound: optional.Some(mustAdministrativePnl(t, "-10")),
	}
	policy := policies.BuildSpotFundsPnlBoundsKillSwitch().AccountBarriers(
		policies.SpotFundsPnlBoundsAccountBarrier{
			AccountID: administrativeChainAccount(),
			Barrier:   barrier,
		},
	)
	return buildAdministrativePnlChainNativeEngine(t, policy.Build)
}

func buildAdministrativePnlChainNativeEngine(
	t *testing.T,
	build func(native.EngineBuilder) error,
) native.Engine {
	t.Helper()
	builder, err := native.CreateEngineBuilder(native.SyncPolicyAccount)
	if err != nil {
		t.Fatalf("CreateEngineBuilder() error = %v", err)
	}
	if err := build(builder); err != nil {
		native.DestroyEngineBuilder(builder)
		t.Fatalf("BuildSpotFundsPnlBoundsKillSwitch() error = %v", err)
	}
	engine, buildErr, err := native.EngineBuilderBuild(builder)
	native.DestroyEngineBuilder(builder)
	if buildErr != nil {
		native.DestroyEngineBuildError(buildErr)
		t.Fatal("EngineBuilderBuild() returned a build error")
	}
	if err != nil {
		t.Fatalf("EngineBuilderBuild() error = %v", err)
	}
	t.Cleanup(func() { native.DestroyEngine(engine) })
	return engine
}

func mustAdministrativePnl(t *testing.T, value string) param.Pnl {
	t.Helper()
	pnl, err := param.NewPnlFromString(value)
	if err != nil {
		t.Fatalf("NewPnlFromString(%q) error = %v", value, err)
	}
	return pnl
}

func verifyAdministrativePnlBlocks(
	t *testing.T,
	driver *administrativeChainDriver,
	state *administrativeChainState,
) {
	t.Helper()
	if len(state.blocks) != 1 {
		t.Fatalf("account blocks = %v, want one", state.blocks)
	}
	block := state.blocks[0]
	if block.Code != reject.CodePnlKillSwitchTriggered ||
		block.Policy != policies.SpotFundsPolicyName {
		t.Fatalf(
			"account block = %+v, want SpotFunds P&L kill-switch block",
			block,
		)
	}
	wantReason := "pnl kill switch triggered"
	wantDetails := []string{"lower bound breached", "realized pnl -20", "currency USD"}
	if state.assignment.State.IsHalted() {
		wantReason = "account pnl calculation halted"
		wantDetails = []string{"halt reason MissingFx"}
	}
	if block.Reason != wantReason {
		t.Errorf("account block reason = %q, want %q", block.Reason, wantReason)
	}
	for _, want := range wantDetails {
		if !strings.Contains(block.Details, want) {
			t.Errorf("account block details = %q, want %q", block.Details, want)
		}
	}
	request, rejects := observeAdministrativePreTrade(
		t, driver, state.accountIDs[0],
	)
	if request != nil {
		request.Close()
		t.Fatal("target account request is non-nil, want P&L block")
	}
	if len(rejects) != 1 ||
		rejects[0].Code != reject.CodePnlKillSwitchTriggered {
		t.Errorf("target account rejects = %v, want P&L kill-switch reject", rejects)
	}
	verifyAdministrativeAccountPasses(t, driver, state.otherAccount)
}
