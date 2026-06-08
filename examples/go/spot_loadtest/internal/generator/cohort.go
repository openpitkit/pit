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

package generator

import (
	"fmt"
	"math"

	"openpit-loadtest-spot-funds-go/internal/config"
)

// cohort is the runtime form of a configured cohort: the validated config plus
// precomputed selection tables (cumulative size weights, cumulative symbol
// weights for the skew). Building the tables once keeps per-event selection
// cheap and deterministic.
type cohort struct {
	cfg config.Cohort
	// sizeCum is the cumulative weight table over cfg.SizeWeights.
	sizeCum []float64
	// symbolCum is the cumulative weight table over the global symbol list,
	// reflecting this cohort's symbol skew (uniform or Zipf).
	symbolCum []float64
}

// account is one population member bound to a cohort.
type account struct {
	// id is the engine-facing account key string (FNV-hashed by the binding).
	id string
	// cohort indexes into population.cohorts.
	cohort int
}

// population is the partitioned account/instrument universe.
type population struct {
	cohorts []cohort
	// cohortCum is the cumulative cohort-weight table used to assign accounts.
	cohortCum []float64
	accounts  []account
	symbols   []string
	// rejectCum is the cumulative reject-propensity table over cohorts, used by
	// the reject controller to pick which cohort absorbs a forced reject.
	rejectCum []float64

	// byCohort lists the account indices belonging to each cohort, in ascending
	// index order. Used by the bounded-concurrency scheduler to admit accounts
	// into the active working set.
	byCohort [][]int
	// admitCum is the cumulative admission-weight table over cohorts. The
	// admission weight is Weight*Activity, so chatty cohorts (high population
	// share AND high chattiness) wake far more often than dormant ones, while
	// the dormant majority is admitted only rarely.
	admitCum []float64
}

// buildPopulation partitions accountCount accounts across the configured
// cohorts by weight and precomputes every selection table. Account assignment
// is deterministic: accounts are dealt to cohorts in proportion to weight using
// a largest-remainder split, then laid out in cohort order, so account i's
// cohort depends only on the config (not on the RNG) - keeping the stream
// stable even if account iteration order changes elsewhere.
func buildPopulation(cfg *config.Config) (*population, error) {
	symbols := cfg.Instruments.Symbols
	if len(symbols) == 0 {
		return nil, fmt.Errorf("population: no instruments")
	}

	cohorts := make([]cohort, len(cfg.Cohorts))
	weights := make([]float64, len(cfg.Cohorts))
	rejectWeights := make([]float64, len(cfg.Cohorts))
	for i, cc := range cfg.Cohorts {
		sizeWeights := make([]float64, len(cc.SizeWeights))
		for j, b := range cc.SizeWeights {
			sizeWeights[j] = b.Weight
		}
		symbolCum, err := symbolWeights(cc, symbols)
		if err != nil {
			return nil, fmt.Errorf("cohort %q: %w", cc.Name, err)
		}
		cohorts[i] = cohort{
			cfg:       cc,
			sizeCum:   cumulative(sizeWeights),
			symbolCum: symbolCum,
		}
		weights[i] = cc.Weight
		rejectWeights[i] = cc.RejectPropensity
	}

	accounts, err := assignAccounts(cfg.Accounts.Count, weights)
	if err != nil {
		return nil, err
	}

	// If every cohort has zero reject propensity, fall back to uniform so the
	// controller can still place forced rejects somewhere.
	if sum(rejectWeights) == 0 {
		for i := range rejectWeights {
			rejectWeights[i] = 1
		}
	}

	// Index accounts by cohort and build the admission weight table for the
	// bounded-concurrency scheduler.
	byCohort := make([][]int, len(cohorts))
	for i := range accounts {
		c := accounts[i].cohort
		byCohort[c] = append(byCohort[c], i)
	}
	admitWeights := make([]float64, len(cohorts))
	for i, cc := range cfg.Cohorts {
		// A cohort with no accounts must never be admitted; zero its weight so
		// the scheduler never selects an empty cohort.
		if len(byCohort[i]) == 0 {
			admitWeights[i] = 0
			continue
		}
		admitWeights[i] = cc.Weight * cc.Activity
	}
	// If every admission weight is zero (e.g. all activity == 0), fall back to
	// population share so the scheduler can still admit accounts.
	if sum(admitWeights) == 0 {
		for i := range admitWeights {
			if len(byCohort[i]) > 0 {
				admitWeights[i] = weights[i]
			}
		}
	}

	return &population{
		cohorts:   cohorts,
		cohortCum: cumulative(weights),
		accounts:  accounts,
		symbols:   symbols,
		rejectCum: cumulative(rejectWeights),
		byCohort:  byCohort,
		admitCum:  cumulative(admitWeights),
	}, nil
}

// admitAccount draws one account index to admit into the active working set,
// weighted by cohort admission weight (chatty cohorts dominate). The caller
// supplies an "is active" predicate; if the weighted pick lands on an already
// active account, selection advances linearly within that cohort's account list
// to the next inactive one (deterministic and guaranteed to make progress while
// any inactive account exists). Returns -1 only when every account is active,
// which the scheduler prevents by keeping active_accounts < len(accounts) or by
// admitting only into free slots.
func (p *population) admitAccount(g *rng, active func(int) bool) int {
	cohortIdx := g.pickWeighted(p.admitCum)
	list := p.byCohort[cohortIdx]
	if len(list) == 0 {
		return p.scanInactive(active)
	}
	start := g.intn(len(list))
	for off := 0; off < len(list); off++ {
		idx := list[(start+off)%len(list)]
		if !active(idx) {
			return idx
		}
	}
	// This cohort is fully active; fall back to a global scan so admission still
	// makes progress when an inactive account exists in another cohort.
	return p.scanInactive(active)
}

// scanInactive returns the lowest-index inactive account, or -1 if all are
// active. The deterministic fallback for a saturated admission draw.
func (p *population) scanInactive(active func(int) bool) int {
	for i := range p.accounts {
		if !active(i) {
			return i
		}
	}
	return -1
}

// assignAccounts deals count accounts to cohorts proportional to weights using
// the largest-remainder method, guaranteeing the counts sum to exactly count
// and that every cohort with positive weight gets at least its floor share.
func assignAccounts(count uint64, weights []float64) ([]account, error) {
	if count == 0 {
		return nil, fmt.Errorf("population: account count must be > 0")
	}
	total := sum(weights)
	if total <= 0 {
		return nil, fmt.Errorf("population: total cohort weight must be > 0")
	}

	n := int(count) //nolint:gosec // count is a configured population size, bounded well below int max
	quotas := make([]float64, len(weights))
	counts := make([]int, len(weights))
	assigned := 0
	for i, w := range weights {
		q := float64(n) * w / total
		quotas[i] = q
		counts[i] = int(math.Floor(q))
		assigned += counts[i]
	}
	// Distribute the remainder to the largest residuals (quota minus already
	// assigned), deterministic on ties by lower index.
	for ; assigned < n; assigned++ {
		best, bestResidual := -1, math.Inf(-1)
		for i := range weights {
			residual := quotas[i] - float64(counts[i])
			if residual > bestResidual {
				bestResidual, best = residual, i
			}
		}
		counts[best]++
	}

	accounts := make([]account, 0, n)
	idx := 0
	for ci, c := range counts {
		for k := 0; k < c; k++ {
			accounts = append(accounts, account{
				id:     fmt.Sprintf("acct-%06d", idx),
				cohort: ci,
			})
			idx++
		}
	}
	return accounts, nil
}

// symbolWeights builds the cumulative symbol-selection table for a cohort. For
// uniform skew every symbol has weight 1; for Zipf the i-th symbol (1-based in
// list order) has weight 1/i^s, biasing toward the head of the symbol list.
func symbolWeights(cc config.Cohort, symbols []string) ([]float64, error) {
	w := make([]float64, len(symbols))
	switch cc.SymbolSkew {
	case config.SymbolSkewUniform:
		for i := range w {
			w[i] = 1
		}
	case config.SymbolSkewZipf:
		for i := range w {
			w[i] = 1.0 / math.Pow(float64(i+1), cc.ZipfS)
		}
	default:
		return nil, fmt.Errorf("unknown symbol_skew %q", cc.SymbolSkew)
	}
	return cumulative(w), nil
}

// pickSymbol returns a symbol index for the cohort using its skew table.
func (p *population) pickSymbol(g *rng, cohortIdx int) int {
	return g.pickWeighted(p.cohorts[cohortIdx].symbolCum)
}

// pickSize returns the order quantity (lots) for the cohort using its size
// distribution.
func (p *population) pickSize(g *rng, cohortIdx int) uint64 {
	idx := g.pickWeighted(p.cohorts[cohortIdx].sizeCum)
	return p.cohorts[cohortIdx].cfg.SizeWeights[idx].Quantity
}

func sum(xs []float64) float64 {
	var s float64
	for _, x := range xs {
		s += x
	}
	return s
}
