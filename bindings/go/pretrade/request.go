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

package pretrade

import (
	"fmt"

	"go.openpit.dev/openpit/internal/native"
	"go.openpit.dev/openpit/reject"
)

type Request struct {
	handle native.PretradePreTradeRequest
}

func NewRequestFromHandle(handle native.PretradePreTradeRequest) *Request {
	return &Request{handle: handle}
}

// Close releases the request.
//
// The request is owned by the caller from the moment the engine returns
// it. Close must be called once when the caller is done with it — after
// Execute (regardless of its outcome) or when the request is abandoned
// without ever calling Execute.
//
// Idempotency: safe to call more than once; subsequent calls are no-ops.
func (r *Request) Close() {
	native.DestroyPretradePreTradeRequest(r.handle)
	r.handle = nil
}

// Execute runs the main stage of the pre-trade pipeline against this
// request and, on accept, produces a reservation.
//
// Execute does not close the request; the caller must still call Close on
// it afterwards, regardless of Execute's outcome.
//
// A request can be executed at most once; a second call to Execute
// returns a transport error.
func (r *Request) Execute() (*Reservation, reject.List, error) {
	reservation, rejects, err := native.PretradePreTradeRequestExecute(r.handle)
	if err != nil {
		return nil, nil, err
	}

	if rejects != nil {
		reject_result, err := reject.NewListFromHandle(rejects)
		native.DestroyRejectList(rejects)
		if err != nil {
			return nil,
				nil,
				fmt.Errorf("failed to create reject list for rejected order: %w", err)
		}
		return nil, reject_result, nil
	}

	return NewReservationFromHandle(reservation), nil, nil
}
