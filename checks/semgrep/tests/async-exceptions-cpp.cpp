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

void Dropped() {
  // ruleid: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
  }
}

void Swallowed() {
  // ruleid: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    StopWorker();
  }
}

void Rethrown() {
  // ok: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    throw;
  }
}

void Terminated() {
  // ok: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    std::terminate();
  }
}

void Captured() {
  // ok: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    failure = std::current_exception();
  }
}

void ExceptionRethrown() {
  // ok: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    std::rethrow_exception(failure);
  }
}

void Reported() {
  // ok: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    promise.Fail(Error());
  }
}

void ExceptionReported() {
  // ok: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    promise.set_exception(failure);
  }
}

void ReportingInsideNoexcept() {
  // ok: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    [&]() noexcept { promise.Fail(Error()); }();
  }
}

void MisleadingText() {
  // ruleid: async-cpp-dropped-exception
  try {
    Work();
  } catch (...) {
    Log("std::terminate() or promise.Fail(error)");
  }
}

void AfterStandardHandler() {
  // ruleid: async-cpp-dropped-exception
  try {
    Work();
  } catch (const std::exception &ex) {
    Report(ex);
  } catch (...) {
  }
}
