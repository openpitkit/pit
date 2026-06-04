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

package asyncengine

import (
	"context"

	"go.openpit.dev/openpit/param"
	"go.openpit.dev/openpit/pkg/future"
	"go.openpit.dev/openpit/pretrade"
	"go.openpit.dev/openpit/reject"
)

// AsyncRequest wraps a *pretrade.Request returned by StartPreTrade so
// that Execute and Close are routed through the same per-account queue
// as the StartPreTrade call that produced the request. This preserves
// the AccountSync invariant across the Start - Execute boundary while
// still letting the caller perform unrelated work between the two
// stages.
//
// Calling Execute more than once, or using the request after Close, is a
// programmer error: the second call operates on an already-consumed or
// closed underlying *pretrade.Request, mirroring the synchronous
// contract that a request can be executed at most once.
type AsyncRequest struct {
	inner     *pretrade.Request
	engine    *AsyncEngine
	accountID param.AccountID
}

func newAsyncRequest(
	inner *pretrade.Request,
	engine *AsyncEngine,
	accountID param.AccountID,
) *AsyncRequest {
	return &AsyncRequest{
		inner:     inner,
		engine:    engine,
		accountID: accountID,
	}
}

// Execute enqueues the main-stage call against this request. The result
// future mirrors the synchronous main-stage tuple (see
// AsyncEngine.ExecutePreTrade): a non-nil *AsyncReservation on accept, a
// non-nil rejects slice on a policy reject, or a set error on transport
// failure.
//
// Execute always closes the underlying *pretrade.Request once the
// main-stage call has been issued, mirroring the contract that callers
// must Close the request afterwards regardless of outcome.
func (r *AsyncRequest) Execute(
	ctx context.Context,
) *future.Future2[*AsyncReservation, []reject.Reject] {
	f := future.New2[*AsyncReservation, []reject.Reject]()
	task := &executeRequestTask{f: f, req: r}
	if err := r.engine.strategy.submit(
		ctx, r.accountID, task,
	); err != nil {
		r.inner.Close()
		f.Resolve(nil, nil, err)
	}
	return f
}

// executeRequestTask carries one AsyncRequest.Execute call to its worker.
// run always closes the underlying request after the main stage is issued;
// abort closes it too, so an aborted Execute never leaks the native handle.
type executeRequestTask struct {
	f   *future.Future2[*AsyncReservation, []reject.Reject]
	req *AsyncRequest
}

func (t *executeRequestTask) run() {
	r := t.req
	defer r.inner.Close()
	reservation, rejects, err := r.inner.Execute()
	if err != nil {
		t.f.Resolve(nil, nil, err)
		return
	}
	if rejects != nil {
		t.f.Resolve(nil, rejects, nil)
		return
	}
	t.f.Resolve(newAsyncReservation(reservation, r.engine, r.accountID), nil, nil)
}

func (t *executeRequestTask) abort(err error) {
	t.req.inner.Close()
	t.f.Resolve(nil, nil, err)
}

// Close enqueues the release of the underlying request without running
// the main stage. Use it to abandon a request that should not be
// executed.
//
// Close still serializes through the account's queue so concurrent
// engine calls on the same account remain disallowed.
func (r *AsyncRequest) Close(ctx context.Context) *future.Future[struct{}] {
	f := future.New[struct{}]()
	task := &closeRequestTask{f: f, req: r}
	if err := r.engine.strategy.submit(
		ctx, r.accountID, task,
	); err != nil {
		r.inner.Close()
		f.Resolve(struct{}{}, err)
	}
	return f
}

// closeRequestTask carries one AsyncRequest.Close call to its worker. Both
// run and abort close the underlying request so the native handle is
// always released.
type closeRequestTask struct {
	f   *future.Future[struct{}]
	req *AsyncRequest
}

func (t *closeRequestTask) run() {
	t.req.inner.Close()
	t.f.Resolve(struct{}{}, nil)
}

func (t *closeRequestTask) abort(err error) {
	t.req.inner.Close()
	t.f.Resolve(struct{}{}, err)
}

// AccountID returns the account identifier the request is pinned to.
func (r *AsyncRequest) AccountID() param.AccountID { return r.accountID }
