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
	"testing"
)

func TestMapParamErrorMessage(t *testing.T) {
	t.Parallel()

	testCases := []struct {
		name      string
		message   string
		expected  error
		substring string
	}{
		{
			name:     "negative",
			message:  "invalid typed param.Quantity value: value must be non-negative",
			expected: ErrNegative,
		},
		{
			name:     "division by zero",
			message:  "division by zero",
			expected: ErrDivisionByZero,
		},
		{
			name:     "overflow",
			message:  "arithmetic overflow while multiplying",
			expected: ErrOverflow,
		},
		{
			name:     "underflow",
			message:  "arithmetic underflow",
			expected: ErrUnderflow,
		},
		{
			name:     "invalid float",
			message:  "invalid float value (NaN or infinity)",
			expected: ErrInvalidFloat,
		},
		{
			name:     "invalid format",
			message:  "invalid format",
			expected: ErrInvalidFormat,
		},
		{
			name:     "invalid price",
			message:  "invalid price value",
			expected: ErrInvalidPrice,
		},
		{
			name:     "invalid leverage",
			message:  "invalid leverage value",
			expected: ErrInvalidLeverage,
		},
		{
			name:      "fallback wraps message",
			message:   "something else",
			substring: "param: something else",
		},
	}

	for _, testCase := range testCases {
		testCase := testCase
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			actual := mapParamErrorMessage(testCase.message)
			if testCase.expected != nil {
				if !errors.Is(actual, testCase.expected) {
					t.Fatalf("unexpected mapped error: %v", actual)
				}
				return
			}
			if actual == nil {
				t.Fatalf("expected wrapped error")
			}
			if actual.Error() != testCase.substring {
				t.Fatalf("unexpected wrapped message: %q", actual.Error())
			}
		})
	}
}
