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

package accountadjustment

import (
	"go.openpit.dev/openpit/pkg/optional"
	"go.openpit.dev/openpit/reject"
)

// BatchResult reports the completed account-adjustment batch.
//
// On rejection BatchError is set and Outcomes and AccountBlocks are empty. On
// acceptance AccountBlocks contains blocks recorded by the engine after all
// batch mutations committed.
type BatchResult struct {
	BatchError    optional.Option[reject.AccountAdjustmentBatchError]
	Outcomes      []Outcome
	AccountBlocks []reject.AccountBlock
}
