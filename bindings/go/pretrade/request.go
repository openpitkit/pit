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

package pretrade

import (
	"errors"
	"fmt"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/reject"
)

// ErrRequestClosed is returned by Execute once the request has been released.
var ErrRequestClosed = errors.New("pre-trade request already closed")

// Request is a pre-trade check request returned by the engine.
//
// Lifecycle: the caller owns the request and must release it with Close, after
// Execute or when the request is abandoned. Nothing releases it on the caller's
// behalf - no caller-owned handle in this binding has a finalizer, so a request
// dropped without Close leaks its native handle.
//
// Concurrency: callers must serialize methods on the same request. Distinct
// request handles may be used concurrently when the engine sync mode permits.
type Request struct {
	handle native.PretradePreTradeRequest
}

// NewRequestFromHandle creates a Request from a native handle.
func NewRequestFromHandle(handle native.PretradePreTradeRequest) *Request {
	return &Request{handle: handle}
}

// IsClosed reports whether the request no longer owns a native handle.
func (r *Request) IsClosed() bool {
	return r.handle == nil
}

// Close releases the request.
//
// The request is owned by the caller from the moment the engine returns
// it. Close must be called once when the caller is done with it - after
// Execute (regardless of its outcome) or when the request is abandoned
// without ever calling Execute.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (r *Request) Close() {
	if r.handle == nil {
		return
	}
	native.DestroyPretradePreTradeRequest(r.handle)
	r.handle = nil
}

// Execute runs the main stage of the pre-trade pipeline against this
// request and, on accept, produces a reservation.
//
// Execute does not close the request; the caller must still call Close on
// it afterwards, regardless of Execute's outcome.
//
// A request can be executed at most once. A call after Close returns
// [ErrRequestClosed]. A second Execute that reaches the native request returns
// the native execution error instead.
func (r *Request) Execute() (*Reservation, []reject.Reject, error) {
	if r.handle == nil {
		return nil, nil, ErrRequestClosed
	}
	reservation, rejects, err := native.PretradePreTradeRequestExecute(r.handle)
	if err != nil {
		return nil, nil, err
	}

	if rejects != nil {
		rejectResult, err := reject.NewListFromHandle(rejects)
		native.DestroyPretradeRejectList(rejects)
		if err != nil {
			return nil,
				nil,
				fmt.Errorf("failed to create reject list for rejected order: %w", err)
		}
		return nil, rejectResult, nil
	}

	return NewReservationFromHandle(reservation), nil, nil
}
