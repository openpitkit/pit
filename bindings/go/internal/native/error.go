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

var paramErrorByCode = map[ParamErrorCode]error{
	ParamErrorCodeNegative:        ErrNegative,
	ParamErrorCodeDivisionByZero:  ErrDivisionByZero,
	ParamErrorCodeOverflow:        ErrOverflow,
	ParamErrorCodeUnderflow:       ErrUnderflow,
	ParamErrorCodeInvalidFloat:    ErrInvalidFloat,
	ParamErrorCodeInvalidFormat:   ErrInvalidFormat,
	ParamErrorCodeInvalidPrice:    ErrInvalidPrice,
	ParamErrorCodeInvalidLeverage: ErrInvalidLeverage,
}

func consumeSharedStringAsError(handle SharedString, fallback string, args ...any) error {
	msg := consumeSharedString(handle)
	if msg != "" {
		return errors.New(msg)
	}
	return fmt.Errorf(fallback, args...)
}

func consumeParamError(handle ParamErrorHandle, fallback string, args ...any) error {
	if handle == nil {
		return fmt.Errorf(fallback, args...)
	}
	code := handle.code
	msg := ""
	if handle.message != nil {
		msg = newStringView(C.pit_shared_string_view(handle.message)).Safe()
	}
	C.pit_destroy_param_error(handle)

	sentinel := paramErrorByCode[code]
	if sentinel != nil {
		if msg != "" {
			return fmt.Errorf("%w: %s", sentinel, msg)
		}
		return sentinel
	}
	if msg != "" {
		return fmt.Errorf("param: %s", msg)
	}
	return fmt.Errorf(fallback, args...)
}
