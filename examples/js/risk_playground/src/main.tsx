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

import { createRoot } from "react-dom/client";

import { App } from "./App.tsx";
import { initializeAnalytics } from "./analytics.ts";
import "./design-system/tokens/colors.css";
import "./design-system/tokens/typography.css";
import "./design-system/tokens/spacing.css";
import "./design-system/tokens/finance.css";
import "./styles.css";

initializeAnalytics();
const container = document.getElementById("app");
if (container === null) {
  throw new Error("the risk playground could not initialize");
}

createRoot(container).render(<App />);
