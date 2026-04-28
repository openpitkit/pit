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

package native

import (
	"errors"
	"strings"
	"testing"
)

func TestConsumeParamErrorMapsSentinelByCode(t *testing.T) {
	t.Parallel()

	_, err := CreateParamQuantityFromF64(-1)
	if !errors.Is(err, ErrNegative) {
		t.Fatalf("expected ErrNegative, got %v", err)
	}
	if err == nil || !strings.Contains(err.Error(), "value must be non-negative") {
		t.Fatalf("expected wrapped error message, got %v", err)
	}
}

func TestConsumeParamErrorFallbackForUnspecifiedCode(t *testing.T) {
	t.Parallel()

	_, err := CreateParamPnl(ParamDecimal{
		mantissa_lo: 1,
		mantissa_hi: 0,
		scale:       -1,
	})
	if err == nil {
		t.Fatalf("expected error")
	}
	if !strings.HasPrefix(err.Error(), "param: ") {
		t.Fatalf("expected generic param prefix, got %q", err.Error())
	}
}

func TestConsumeParamErrorFallbackWhenHandleIsNil(t *testing.T) {
	t.Parallel()

	err := consumeParamError(nil, "fallback %d", 42)
	if err == nil {
		t.Fatalf("expected fallback error")
	}
	if err.Error() != "fallback 42" {
		t.Fatalf("unexpected fallback message: %q", err.Error())
	}
}
