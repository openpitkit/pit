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

// Package future provides the abstract future/promise primitives shared
// across the SDK. They are deliberately transport-agnostic and carry no
// engine-specific knowledge, so any package can depend on them without
// pulling in the async engine.
package future

import (
	"context"
	"sync"
)

// Future represents the eventual result of an asynchronous operation that
// yields a single value. A Future is resolved exactly once with either a
// value or an error; further resolution attempts are silently ignored.
//
// Futures are safe for concurrent use by multiple goroutines. Multiple
// goroutines may block on Await for the same Future; all of them observe
// the same value and error after resolution.
//
// A Future handed back to a caller is meant to be read-only: producers create
// it with New and complete it with Resolve, consumers observe it with Await,
// Done, TryGet, and Wait.
type Future[T any] struct {
	done  chan struct{}
	once  sync.Once
	value T
	err   error
}

// New creates an unresolved Future.
func New[T any]() *Future[T] {
	return &Future[T]{done: make(chan struct{})}
}

// Resolve completes the future with the supplied value and error. Only the
// first call has an effect.
func (f *Future[T]) Resolve(value T, err error) {
	f.once.Do(
		func() {
			f.value = value
			f.err = err
			close(f.done)
		},
	)
}

// Await blocks until the future is resolved or ctx is cancelled. On ctx
// cancellation the future's resolution is unaffected; the caller simply
// gives up waiting and receives the zero value with ctx.Err().
func (f *Future[T]) Await(ctx context.Context) (T, error) {
	select {
	case <-f.done:
		return f.value, f.err
	case <-ctx.Done():
		var zero T
		return zero, ctx.Err()
	}
}

// Done reports whether the future has been resolved.
func (f *Future[T]) Done() bool {
	select {
	case <-f.done:
		return true
	default:
		return false
	}
}

// TryGet returns the resolved value and error without blocking. If the
// future is not yet resolved, ok is false and value/err are the zero
// values.
func (f *Future[T]) TryGet() (value T, ok bool, err error) {
	select {
	case <-f.done:
		return f.value, true, f.err
	default:
		var zero T
		return zero, false, nil
	}
}

// Wait returns a channel that is closed once the future is resolved.
// Useful for composing futures with other select statements.
func (f *Future[T]) Wait() <-chan struct{} {
	return f.done
}

// pair bundles the two values a Future2 carries so the resolution machinery
// can ride on a plain Future without duplicating it.
type pair[A, B any] struct {
	first  A
	second B
}

// Future2 represents the eventual result of an asynchronous operation that
// yields two values alongside an error, mirroring a Go multi-return tuple.
// It exists for operations whose synchronous counterpart returns two values
// and an error (for example "request-or-rejects" and "reservation-or-rejects"),
// so the asynchronous and synchronous shapes line up exactly instead of
// forcing callers through a result struct.
//
// Future2 is a thin adapter over Future[pair]: it parameterizes the ordinary
// future with a two-field value and splits that value back into two results at
// the read boundary, so all concurrency and once-resolution semantics are
// inherited rather than reimplemented.
type Future2[A, B any] struct {
	inner *Future[pair[A, B]]
}

// New2 creates an unresolved Future2.
func New2[A, B any]() *Future2[A, B] {
	return &Future2[A, B]{inner: New[pair[A, B]]()}
}

// Resolve completes the future with the supplied values and error. Only the
// first call has an effect.
func (f *Future2[A, B]) Resolve(first A, second B, err error) {
	f.inner.Resolve(pair[A, B]{first: first, second: second}, err)
}

// Await blocks until the future is resolved or ctx is cancelled. On ctx
// cancellation the caller receives zero values with ctx.Err().
func (f *Future2[A, B]) Await(ctx context.Context) (A, B, error) {
	value, err := f.inner.Await(ctx)
	return value.first, value.second, err
}

// Done reports whether the future has been resolved.
func (f *Future2[A, B]) Done() bool {
	return f.inner.Done()
}

// TryGet returns the resolved values and error without blocking. If the
// future is not yet resolved, ok is false and the values/err are zero.
func (f *Future2[A, B]) TryGet() (first A, second B, ok bool, err error) {
	value, ok, err := f.inner.TryGet()
	return value.first, value.second, ok, err
}

// Wait returns a channel that is closed once the future is resolved.
func (f *Future2[A, B]) Wait() <-chan struct{} {
	return f.inner.Wait()
}
