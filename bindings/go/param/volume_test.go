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

package param

import "testing"

func TestVolumeCalculateQuantity(t *testing.T) {
	volume, err := NewVolumeFromString("6")
	if err != nil {
		t.Fatalf("NewVolumeFromString() error = %v", err)
	}

	tests := []struct {
		name  string
		price string
		want  string
	}{
		{name: "positive price", price: "2", want: "3"},
		{name: "negative price", price: "-2", want: "3"},
		{name: "zero price", price: "0", want: "0"},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			price, err := NewPriceFromString(test.price)
			if err != nil {
				t.Fatalf("NewPriceFromString() error = %v", err)
			}

			quantity, err := volume.CalculateQuantity(price)
			if err != nil {
				t.Fatalf("CalculateQuantity() error = %v", err)
			}
			if got := quantity.String(); got != test.want {
				t.Fatalf("CalculateQuantity() = %q, want %q", got, test.want)
			}
		})
	}
}
