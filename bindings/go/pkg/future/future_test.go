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

package future

import (
	"context"
	"errors"
	"testing"
	"time"
)

func TestFutureResolveBeforeAwait(t *testing.T) {
	t.Parallel()
	f := New[int]()
	f.Resolve(42, nil)

	got, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() error = %v", err)
	}
	if got != 42 {
		t.Fatalf("Await() = %d, want 42", got)
	}
}

func TestFutureResolveDuringAwait(t *testing.T) {
	t.Parallel()
	f := New[string]()
	go func() {
		time.Sleep(10 * time.Millisecond)
		f.Resolve("ok", nil)
	}()
	got, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() error = %v", err)
	}
	if got != "ok" {
		t.Fatalf("Await() = %q, want %q", got, "ok")
	}
}

func TestFutureCtxCancelDuringAwait(t *testing.T) {
	t.Parallel()
	f := New[int]()
	ctx, cancel := context.WithTimeout(
		context.Background(), 5*time.Millisecond,
	)
	defer cancel()
	got, err := f.Await(ctx)
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("Await() err = %v, want DeadlineExceeded", err)
	}
	if got != 0 {
		t.Fatalf("Await() = %d, want zero", got)
	}
}

func TestFutureResolveIsIdempotent(t *testing.T) {
	t.Parallel()
	f := New[int]()
	f.Resolve(1, nil)
	f.Resolve(2, errors.New("ignored"))

	got, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() error = %v", err)
	}
	if got != 1 {
		t.Fatalf("Await() = %d, want 1", got)
	}
}

func TestFutureDoneAndTryGet(t *testing.T) {
	t.Parallel()
	f := New[int]()
	if f.Done() {
		t.Fatalf("Done() = true before resolve")
	}
	if _, ok, _ := f.TryGet(); ok {
		t.Fatalf("TryGet() ok = true before resolve")
	}

	f.Resolve(7, nil)
	if !f.Done() {
		t.Fatalf("Done() = false after resolve")
	}
	value, ok, err := f.TryGet()
	if !ok {
		t.Fatalf("TryGet() ok = false after resolve")
	}
	if err != nil {
		t.Fatalf("TryGet() error = %v", err)
	}
	if value != 7 {
		t.Fatalf("TryGet() value = %d, want 7", value)
	}
}

func TestFutureWaitChannel(t *testing.T) {
	t.Parallel()
	f := New[int]()
	select {
	case <-f.Wait():
		t.Fatalf("Wait() ready before resolve")
	default:
	}
	go f.Resolve(1, nil)
	select {
	case <-f.Wait():
	case <-time.After(time.Second):
		t.Fatalf("Wait() did not signal after resolve")
	}
}

func TestFutureError(t *testing.T) {
	t.Parallel()
	target := errors.New("boom")
	f := New[int]()
	f.Resolve(0, target)
	_, err := f.Await(context.Background())
	if !errors.Is(err, target) {
		t.Fatalf("Await() err = %v, want %v", err, target)
	}
}

func TestFuture2ResolveBeforeAwait(t *testing.T) {
	t.Parallel()
	f := New2[string, int]()
	f.Resolve("ok", 7, nil)

	first, second, err := f.Await(context.Background())
	if err != nil {
		t.Fatalf("Await() error = %v", err)
	}
	if first != "ok" || second != 7 {
		t.Fatalf("Await() = (%q, %d), want (ok, 7)", first, second)
	}
}

func TestFuture2CtxCancelDuringAwait(t *testing.T) {
	t.Parallel()
	f := New2[string, int]()
	ctx, cancel := context.WithTimeout(
		context.Background(), 5*time.Millisecond,
	)
	defer cancel()
	first, second, err := f.Await(ctx)
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Fatalf("Await() err = %v, want DeadlineExceeded", err)
	}
	if first != "" || second != 0 {
		t.Fatalf("Await() = (%q, %d), want zero values", first, second)
	}
}

func TestFuture2DoneAndTryGet(t *testing.T) {
	t.Parallel()
	f := New2[string, int]()
	if f.Done() {
		t.Fatalf("Done() = true before resolve")
	}
	if _, _, ok, _ := f.TryGet(); ok {
		t.Fatalf("TryGet() ok = true before resolve")
	}

	f.Resolve("done", 9, nil)
	if !f.Done() {
		t.Fatalf("Done() = false after resolve")
	}
	first, second, ok, err := f.TryGet()
	if !ok || err != nil || first != "done" || second != 9 {
		t.Fatalf("TryGet() = (%q, %d, %v, %v), want (done, 9, true, nil)", first, second, ok, err)
	}
}
