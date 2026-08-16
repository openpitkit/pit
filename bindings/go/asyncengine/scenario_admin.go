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
	"fmt"

	"go.openpit.dev/openpit/configure"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
)

// BlockHooks prepares and handles an account or account-group block.
type BlockHooks[State any] struct {
	// Reason prepares the block reason from State inside the selected lane.
	Reason func(context.Context, State) (string, error)
	// OnBlocked runs after the block operation returns successfully.
	OnBlocked func(context.Context, State) error
}

// UnblockHooks handles an account or account-group unblock.
type UnblockHooks[State any] struct {
	// OnUnblocked runs after the unblock operation returns successfully.
	OnUnblocked func(context.Context, State) error
}

// RegisterAccountGroupHooks handles account-group registration.
// A successful membership change can invalidate stored P&L and cost basis and
// can latch account blocks before OnRegistered runs.
type RegisterAccountGroupHooks[State any] struct {
	// OnRegistered runs after all accounts are registered successfully and any
	// resulting account blocks have been latched.
	OnRegistered func(context.Context, State) error
}

// UnregisterAccountGroupHooks handles account-group removal.
// A successful membership change can invalidate stored P&L and cost basis and
// can latch account blocks before OnUnregistered runs.
type UnregisterAccountGroupHooks[State any] struct {
	// OnUnregistered runs after all accounts are unregistered successfully and
	// any resulting account blocks have been latched.
	OnUnregistered func(context.Context, State) error
}

// CurrencyHooks prepares and handles an account or account-group currency set.
type CurrencyHooks[State any] struct {
	// Currency prepares the currency from State inside the selected lane.
	Currency func(context.Context, State) (param.Asset, error)
	// OnSet runs after the currency is set successfully.
	OnSet func(context.Context, State) error
}

// ClearCurrencyHooks handles an account or account-group currency clear.
type ClearCurrencyHooks[State any] struct {
	// OnCleared runs after the currency is cleared successfully.
	OnCleared func(context.Context, State) error
}

// SpotFundsAccountPnlAssignment identifies one absolute account P&L state
// assignment for a named SpotFunds policy.
type SpotFundsAccountPnlAssignment struct {
	// PolicyName identifies the registered SpotFunds policy.
	PolicyName string
	// State is the absolute numeric or halted account P&L state to assign.
	State model.PnlState
}

// SpotFundsAccountPnlHooks prepares and handles an absolute account P&L state
// assignment.
type SpotFundsAccountPnlHooks[State any] struct {
	// Assignment prepares the policy name and state inside the account lane.
	Assignment func(
		context.Context,
		State,
	) (SpotFundsAccountPnlAssignment, error)
	// OnSet handles the synchronous configuration result, including any
	// account blocks the engine latched before returning.
	OnSet func(context.Context, State, configure.PolicyConfigurationResult) error
}

// BlockAccount blocks the source account and then runs its result hook.
func (c *ChainBuilder[State]) BlockAccount(
	hooks BlockHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountSource(sourceKind); err != nil {
			return err
		}
		if hooks.Reason == nil {
			return incompleteAdministrativeHook("BlockAccount.Reason")
		}
		if hooks.OnBlocked == nil {
			return incompleteAdministrativeHook("BlockAccount.OnBlocked")
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.blockAccount(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

// UnblockAccount unblocks the source account and then runs its result hook.
func (c *ChainBuilder[State]) UnblockAccount(
	hooks UnblockHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountSource(sourceKind); err != nil {
			return err
		}
		if hooks.OnUnblocked == nil {
			return incompleteAdministrativeHook("UnblockAccount.OnUnblocked")
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.unblockAccount(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

// RegisterAccountGroup registers accounts with the source account group. The
// whole chain runs in the first account's lane, matching AsyncAccounts, and
// cannot contain operations routed by the group's administrative key.
//
// A membership change that changes effective currency can invalidate stored
// P&L and cost basis. A changed effective P&L barrier can latch account blocks
// before OnRegistered runs.
func (c *ChainBuilder[State]) RegisterAccountGroup(
	accountIDs []param.AccountID,
	hooks RegisterAccountGroupHooks[State],
) *ChainBuilder[State] {
	accountIDs = append([]param.AccountID(nil), accountIDs...)
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountGroupMembershipSource(sourceKind); err != nil {
			return err
		}
		if len(accountIDs) == 0 {
			return ErrMissingAccountID
		}
		if hooks.OnRegistered == nil {
			return incompleteAdministrativeHook("RegisterAccountGroup.OnRegistered")
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.registerAccountGroup(ctx, state, accountIDs, hooks)
	}
	c.steps = append(
		c.steps,
		newAccountGroupMembershipStep(accountIDs, validate, execute),
	)
	return c
}

// UnregisterAccountGroup unregisters accounts from the source account group.
// The whole chain runs in the first account's lane, matching AsyncAccounts,
// and cannot contain operations routed by the group's administrative key.
//
// A membership change that changes effective currency can invalidate stored
// P&L and cost basis. A changed effective P&L barrier can latch account blocks
// before OnUnregistered runs.
func (c *ChainBuilder[State]) UnregisterAccountGroup(
	accountIDs []param.AccountID,
	hooks UnregisterAccountGroupHooks[State],
) *ChainBuilder[State] {
	accountIDs = append([]param.AccountID(nil), accountIDs...)
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountGroupMembershipSource(sourceKind); err != nil {
			return err
		}
		if len(accountIDs) == 0 {
			return ErrMissingAccountID
		}
		if hooks.OnUnregistered == nil {
			return incompleteAdministrativeHook(
				"UnregisterAccountGroup.OnUnregistered",
			)
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.unregisterAccountGroup(ctx, state, accountIDs, hooks)
	}
	c.steps = append(
		c.steps,
		newAccountGroupMembershipStep(accountIDs, validate, execute),
	)
	return c
}

// BlockAccountGroup blocks the source account group.
func (c *ChainBuilder[State]) BlockAccountGroup(
	hooks BlockHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountGroupSource(sourceKind); err != nil {
			return err
		}
		if hooks.Reason == nil {
			return incompleteAdministrativeHook("BlockAccountGroup.Reason")
		}
		if hooks.OnBlocked == nil {
			return incompleteAdministrativeHook("BlockAccountGroup.OnBlocked")
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.blockAccountGroup(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

// UnblockAccountGroup unblocks the source account group.
func (c *ChainBuilder[State]) UnblockAccountGroup(
	hooks UnblockHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountGroupSource(sourceKind); err != nil {
			return err
		}
		if hooks.OnUnblocked == nil {
			return incompleteAdministrativeHook(
				"UnblockAccountGroup.OnUnblocked",
			)
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.unblockAccountGroup(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

// SetAccountCurrency sets the source account's explicit currency.
func (c *ChainBuilder[State]) SetAccountCurrency(
	hooks CurrencyHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountSource(sourceKind); err != nil {
			return err
		}
		if hooks.Currency == nil {
			return incompleteAdministrativeHook("SetAccountCurrency.Currency")
		}
		if hooks.OnSet == nil {
			return incompleteAdministrativeHook("SetAccountCurrency.OnSet")
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.setAccountCurrency(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

// ClearAccountCurrency clears the source account's explicit currency.
func (c *ChainBuilder[State]) ClearAccountCurrency(
	hooks ClearCurrencyHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountSource(sourceKind); err != nil {
			return err
		}
		if hooks.OnCleared == nil {
			return incompleteAdministrativeHook("ClearAccountCurrency.OnCleared")
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.clearAccountCurrency(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

// SetAccountGroupCurrency sets the source account group's currency.
func (c *ChainBuilder[State]) SetAccountGroupCurrency(
	hooks CurrencyHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountGroupSource(sourceKind); err != nil {
			return err
		}
		if hooks.Currency == nil {
			return incompleteAdministrativeHook("SetAccountGroupCurrency.Currency")
		}
		if hooks.OnSet == nil {
			return incompleteAdministrativeHook("SetAccountGroupCurrency.OnSet")
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.setAccountGroupCurrency(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

// ClearAccountGroupCurrency clears the source account group's currency.
func (c *ChainBuilder[State]) ClearAccountGroupCurrency(
	hooks ClearCurrencyHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountGroupSource(sourceKind); err != nil {
			return err
		}
		if hooks.OnCleared == nil {
			return incompleteAdministrativeHook(
				"ClearAccountGroupCurrency.OnCleared",
			)
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.clearAccountGroupCurrency(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

// SetSpotFundsAccountPnl sets the source account's absolute SpotFunds P&L
// state and returns every account block latched by that assignment to its hook.
func (c *ChainBuilder[State]) SetSpotFundsAccountPnl(
	hooks SpotFundsAccountPnlHooks[State],
) *ChainBuilder[State] {
	validate := func(sourceKind chainSourceKind) error {
		if err := requireAccountSource(sourceKind); err != nil {
			return err
		}
		if hooks.Assignment == nil {
			return incompleteAdministrativeHook(
				"SetSpotFundsAccountPnl.Assignment",
			)
		}
		if hooks.OnSet == nil {
			return incompleteAdministrativeHook("SetSpotFundsAccountPnl.OnSet")
		}
		return nil
	}
	execute := func(
		ctx context.Context,
		task *chainTask[State],
		state State,
		_ *pendingFinalizer,
	) (bool, error) {
		return false, task.setSpotFundsAccountPnl(ctx, state, hooks)
	}
	c.steps = append(c.steps, newChainStep(validate, execute))
	return c
}

func requireAccountSource(sourceKind chainSourceKind) error {
	if sourceKind == chainSourceAccountGroup ||
		sourceKind == chainSourceAccountGroupMembership {
		return ErrChainAccountRequired
	}
	return nil
}

func requireAccountGroupSource(sourceKind chainSourceKind) error {
	if sourceKind != chainSourceAccountGroup {
		return ErrChainAccountGroupRequired
	}
	return nil
}

func requireAccountGroupMembershipSource(sourceKind chainSourceKind) error {
	if sourceKind != chainSourceAccountGroupMembership {
		return ErrChainAccountGroupRequired
	}
	return nil
}

func newAccountGroupMembershipStep[State any](
	accountIDs []param.AccountID,
	validate func(chainSourceKind) error,
	execute func(
		context.Context, *chainTask[State], State, *pendingFinalizer,
	) (bool, error),
) chainStep[State] {
	step := newChainStep(validate, execute)
	step.accountGroupMembership = true
	if len(accountIDs) != 0 {
		routingAccountID := accountIDs[0]
		step.routingAccountID = &routingAccountID
	}
	return step
}

func incompleteAdministrativeHook(name string) error {
	return fmt.Errorf("%w: Chain.%s", ErrChainIncomplete, name)
}

func (t *chainTask[State]) blockAccount(
	ctx context.Context,
	state State,
	hooks BlockHooks[State],
) error {
	reason, err := hooks.Reason(ctx, state)
	if err != nil {
		return fmt.Errorf("async chain account block input: %w", err)
	}
	t.retryUnsafe = true
	t.engine.driver.Accounts().Block(t.plan.accountID, reason)
	if err := hooks.OnBlocked(ctx, state); err != nil {
		return fmt.Errorf("async chain account blocked hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) unblockAccount(
	ctx context.Context,
	state State,
	hooks UnblockHooks[State],
) error {
	t.retryUnsafe = true
	t.engine.driver.Accounts().Unblock(t.plan.accountID)
	if err := hooks.OnUnblocked(ctx, state); err != nil {
		return fmt.Errorf("async chain account unblocked hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) registerAccountGroup(
	ctx context.Context,
	state State,
	accountIDs []param.AccountID,
	hooks RegisterAccountGroupHooks[State],
) error {
	if len(accountIDs) == 0 {
		return ErrMissingAccountID
	}
	t.retryUnsafe = true
	if err := t.engine.driver.Accounts().RegisterGroup(
		accountIDs,
		*t.plan.groupID,
	); err != nil {
		return fmt.Errorf("async chain register account group: %w", err)
	}
	if err := hooks.OnRegistered(ctx, state); err != nil {
		return fmt.Errorf("async chain account group registered hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) unregisterAccountGroup(
	ctx context.Context,
	state State,
	accountIDs []param.AccountID,
	hooks UnregisterAccountGroupHooks[State],
) error {
	if len(accountIDs) == 0 {
		return ErrMissingAccountID
	}
	t.retryUnsafe = true
	if err := t.engine.driver.Accounts().UnregisterGroup(
		accountIDs,
		*t.plan.groupID,
	); err != nil {
		return fmt.Errorf("async chain unregister account group: %w", err)
	}
	if err := hooks.OnUnregistered(ctx, state); err != nil {
		return fmt.Errorf("async chain account group unregistered hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) blockAccountGroup(
	ctx context.Context,
	state State,
	hooks BlockHooks[State],
) error {
	reason, err := hooks.Reason(ctx, state)
	if err != nil {
		return fmt.Errorf("async chain account-group block input: %w", err)
	}
	t.retryUnsafe = true
	if err := t.engine.driver.Accounts().BlockGroup(
		*t.plan.groupID,
		reason,
	); err != nil {
		return fmt.Errorf("async chain block account group: %w", err)
	}
	if err := hooks.OnBlocked(ctx, state); err != nil {
		return fmt.Errorf("async chain account group blocked hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) unblockAccountGroup(
	ctx context.Context,
	state State,
	hooks UnblockHooks[State],
) error {
	t.retryUnsafe = true
	if err := t.engine.driver.Accounts().UnblockGroup(*t.plan.groupID); err != nil {
		return fmt.Errorf("async chain unblock account group: %w", err)
	}
	if err := hooks.OnUnblocked(ctx, state); err != nil {
		return fmt.Errorf("async chain account group unblocked hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) setAccountCurrency(
	ctx context.Context,
	state State,
	hooks CurrencyHooks[State],
) error {
	currency, err := hooks.Currency(ctx, state)
	if err != nil {
		return fmt.Errorf("async chain account currency input: %w", err)
	}
	t.retryUnsafe = true
	if err := t.engine.driver.Accounts().SetCurrency(
		t.plan.accountID,
		currency,
	); err != nil {
		return fmt.Errorf("async chain set account currency: %w", err)
	}
	if err := hooks.OnSet(ctx, state); err != nil {
		return fmt.Errorf("async chain account currency set hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) clearAccountCurrency(
	ctx context.Context,
	state State,
	hooks ClearCurrencyHooks[State],
) error {
	t.retryUnsafe = true
	t.engine.driver.Accounts().ClearCurrency(t.plan.accountID)
	if err := hooks.OnCleared(ctx, state); err != nil {
		return fmt.Errorf("async chain account currency cleared hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) setAccountGroupCurrency(
	ctx context.Context,
	state State,
	hooks CurrencyHooks[State],
) error {
	currency, err := hooks.Currency(ctx, state)
	if err != nil {
		return fmt.Errorf("async chain account-group currency input: %w", err)
	}
	t.retryUnsafe = true
	if err := t.engine.driver.Accounts().SetGroupCurrency(
		*t.plan.groupID,
		currency,
	); err != nil {
		return fmt.Errorf("async chain set account-group currency: %w", err)
	}
	if err := hooks.OnSet(ctx, state); err != nil {
		return fmt.Errorf("async chain account-group currency set hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) clearAccountGroupCurrency(
	ctx context.Context,
	state State,
	hooks ClearCurrencyHooks[State],
) error {
	t.retryUnsafe = true
	t.engine.driver.Accounts().ClearGroupCurrency(*t.plan.groupID)
	if err := hooks.OnCleared(ctx, state); err != nil {
		return fmt.Errorf("async chain account-group currency cleared hook: %w", err)
	}
	return nil
}

func (t *chainTask[State]) setSpotFundsAccountPnl(
	ctx context.Context,
	state State,
	hooks SpotFundsAccountPnlHooks[State],
) error {
	assignment, err := hooks.Assignment(ctx, state)
	if err != nil {
		return fmt.Errorf("async chain SpotFunds account P&L input: %w", err)
	}
	t.retryUnsafe = true
	result, err := t.engine.driver.Configure().SetSpotFundsAccountPnl(
		assignment.PolicyName,
		t.plan.accountID,
		assignment.State,
	)
	if err != nil {
		return fmt.Errorf("async chain set SpotFunds account P&L: %w", err)
	}
	if err := hooks.OnSet(ctx, state, result); err != nil {
		return fmt.Errorf("async chain SpotFunds account P&L set hook: %w", err)
	}
	return nil
}
