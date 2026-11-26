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

package policies

import (
	"runtime"
	"slices"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/model"
	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/ptr"
)

// ReviewRuleInstruments selects all instruments or an exact list of
// instruments.
// Its zero value is an empty exact list, which the policy rejects.
type ReviewRuleInstruments struct {
	exact []param.Instrument
	all   bool
}

// ReviewRuleAllInstruments selects every instrument.
func ReviewRuleAllInstruments() ReviewRuleInstruments {
	return ReviewRuleInstruments{all: true}
}

// ReviewRuleExactInstruments selects the listed instruments by both assets.
// An empty list or a repeated instrument is a configuration error.
func ReviewRuleExactInstruments(instruments ...param.Instrument) ReviewRuleInstruments {
	return ReviewRuleInstruments{exact: slices.Clone(instruments)}
}

func (i ReviewRuleInstruments) handle() []native.Instrument {
	if i.all {
		return nil
	}
	out := make([]native.Instrument, len(i.exact))
	for index, instrument := range i.exact {
		out[index] = instrument.Handle()
	}
	return out
}

// ReviewRuleGlobalRule applies to every account on the selected instruments.
type ReviewRuleGlobalRule struct {
	Instruments ReviewRuleInstruments
}

// Handle returns the native rule. Keep the source rule alive until the native
// call returns.
func (r ReviewRuleGlobalRule) Handle() native.ReviewRuleGlobalRule {
	return native.ReviewRuleGlobalRule{Instruments: r.Instruments.handle()}
}

// ReviewRuleAccountGroupRule applies to members of one account group.
type ReviewRuleAccountGroupRule struct {
	Instruments    ReviewRuleInstruments
	AccountGroupID param.AccountGroupID
}

// Handle returns the native rule. Keep the source rule alive until the native
// call returns.
func (r ReviewRuleAccountGroupRule) Handle() native.ReviewRuleAccountGroupRule {
	return native.ReviewRuleAccountGroupRule{
		AccountGroupID: r.AccountGroupID.Handle(),
		Instruments:    r.Instruments.handle(),
	}
}

// ReviewRuleAccountRule applies to one account.
type ReviewRuleAccountRule struct {
	Instruments ReviewRuleInstruments
	AccountID   param.AccountID
}

// Handle returns the native rule. Keep the source rule alive until the native
// call returns.
func (r ReviewRuleAccountRule) Handle() native.ReviewRuleAccountRule {
	return native.ReviewRuleAccountRule{
		AccountID:   r.AccountID.Handle(),
		Instruments: r.Instruments.handle(),
	}
}

// ReviewRuleBuilder is the entry point for a review-rule policy. An axis method
// returns a ReviewRuleReadyBuilder for adding rules and registering the policy.
type ReviewRuleBuilder struct {
	builder *ReviewRuleReadyBuilder
}

// ReviewRuleReadyBuilder holds rules ready for registration with Builtin.
// A matching order yields one ComplianceRestriction per instance. The
// registration mode determines whether that is a reject or a review mark.
// The review-rule policy records no mutations.
type ReviewRuleReadyBuilder struct {
	global        *ReviewRuleGlobalRule
	groups        []ReviewRuleAccountGroupRule
	accounts      []ReviewRuleAccountRule
	policyGroupID model.PolicyGroupID
}

// BuildReviewRule returns a new review-rule policy builder. Calling
// AccountRules or AccountGroupRules with no rules makes an empty policy ready
// for registration. It admits every order and can be configured at runtime.
func BuildReviewRule() *ReviewRuleBuilder {
	return &ReviewRuleBuilder{
		builder: &ReviewRuleReadyBuilder{policyGroupID: model.DefaultPolicyGroupID},
	}
}

// PolicyGroupID assigns a pricing group and returns a ready builder.
func (b *ReviewRuleBuilder) PolicyGroupID(id model.PolicyGroupID) *ReviewRuleReadyBuilder {
	return b.builder.PolicyGroupID(id)
}

// GlobalRule sets the global rule and returns a ready builder.
func (b *ReviewRuleBuilder) GlobalRule(rule ReviewRuleGlobalRule) *ReviewRuleReadyBuilder {
	return b.builder.GlobalRule(rule)
}

// AccountGroupRules adds group rules and returns a ready builder.
func (b *ReviewRuleBuilder) AccountGroupRules(rules ...ReviewRuleAccountGroupRule) *ReviewRuleReadyBuilder {
	return b.builder.AccountGroupRules(rules...)
}

// AccountRules adds account rules and returns a ready builder.
func (b *ReviewRuleBuilder) AccountRules(rules ...ReviewRuleAccountRule) *ReviewRuleReadyBuilder {
	return b.builder.AccountRules(rules...)
}

// PolicyGroupID assigns a pricing group.
// The default is model.DefaultPolicyGroupID.
func (b *ReviewRuleReadyBuilder) PolicyGroupID(id model.PolicyGroupID) *ReviewRuleReadyBuilder {
	b.policyGroupID = id
	return b
}

// GlobalRule sets or replaces the global rule.
func (b *ReviewRuleReadyBuilder) GlobalRule(rule ReviewRuleGlobalRule) *ReviewRuleReadyBuilder {
	b.global = &rule
	return b
}

// AccountGroupRules appends group rules. Each group may occur only once.
func (b *ReviewRuleReadyBuilder) AccountGroupRules(rules ...ReviewRuleAccountGroupRule) *ReviewRuleReadyBuilder {
	b.groups = append(b.groups, rules...)
	return b
}

// AccountRules appends account rules. Each account may occur only once.
func (b *ReviewRuleReadyBuilder) AccountRules(rules ...ReviewRuleAccountRule) *ReviewRuleReadyBuilder {
	b.accounts = append(b.accounts, rules...)
	return b
}

// Build registers the policy on the engine builder. The core rejects repeated
// keys and empty or duplicate-containing exact instrument lists.
func (b *ReviewRuleReadyBuilder) Build(builder native.EngineBuilder, instance *native.PolicyInstance) error {
	var global *native.ReviewRuleGlobalRule
	if b.global != nil {
		global = ptr.New(b.global.Handle())
	}
	groups := make([]native.ReviewRuleAccountGroupRule, len(b.groups))
	for i, rule := range b.groups {
		groups[i] = rule.Handle()
	}
	accounts := make([]native.ReviewRuleAccountRule, len(b.accounts))
	for i, rule := range b.accounts {
		accounts[i] = rule.Handle()
	}
	err := native.EngineBuilderAddBuiltinReviewRule(
		builder, native.PolicyGroupID(b.policyGroupID), global, groups, accounts, instance,
	)
	runtime.KeepAlive(b)
	return err
}
