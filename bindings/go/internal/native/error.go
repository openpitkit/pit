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

/*
#include "pit.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"strings"
)

var (
	ErrNegative        = errors.New("param: value must be non-negative")
	ErrDivisionByZero  = errors.New("param: division by zero")
	ErrOverflow        = errors.New("param: arithmetic overflow")
	ErrUnderflow       = errors.New("param: arithmetic underflow")
	ErrInvalidFloat    = errors.New("param: invalid float value (NaN or infinity)")
	ErrInvalidFormat   = errors.New("param: invalid format")
	ErrInvalidPrice    = errors.New("param: invalid price value")
	ErrInvalidLeverage = errors.New("param: invalid leverage value")
)

func consumeSharedStringAsError(handle SharedString, fallback string, args ...any) error {
	msg := consumeSharedString(handle)
	if msg != "" {
		return errors.New(msg)
	}
	return fmt.Errorf(fallback, args...)
}

func consumeSharedStringAsParamError(handle SharedString, fallback string, args ...any) error {
	msg := consumeSharedString(handle)
	if msg == "" {
		return fmt.Errorf(fallback, args...)
	}
	return mapParamErrorMessage(msg)
}

func mapParamErrorMessage(msg string) error {
	switch {
	case strings.Contains(msg, "value must be non-negative"):
		return ErrNegative
	case strings.Contains(msg, "division by zero"):
		return ErrDivisionByZero
	case strings.Contains(msg, "arithmetic overflow"):
		return ErrOverflow
	case strings.Contains(msg, "arithmetic underflow"):
		return ErrUnderflow
	case strings.Contains(msg, "invalid float value"):
		return ErrInvalidFloat
	case strings.Contains(msg, "invalid format"):
		return ErrInvalidFormat
	case strings.Contains(msg, "invalid price value"):
		return ErrInvalidPrice
	case strings.Contains(msg, "invalid leverage value"):
		return ErrInvalidLeverage
	default:
		return fmt.Errorf("param: %s", msg)
	}
}
