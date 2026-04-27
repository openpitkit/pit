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

use std::str;

use openpit::param::Asset;
use openpit::Instrument;

use crate::PitStringView;

fn parse_string_view(value: PitStringView, field: &str) -> Result<Option<&str>, String> {
    if value.ptr.is_null() {
        if value.len == 0 {
            return Ok(None);
        }
        return Err(format!("{field} pointer is null"));
    }

    let bytes = unsafe { std::slice::from_raw_parts(value.ptr, value.len) };
    let text = str::from_utf8(bytes).map_err(|err| format!("{field} is not valid utf-8: {err}"))?;
    Ok(Some(text))
}

pub(crate) fn parse_asset_view(value: PitStringView, field: &str) -> Result<Option<Asset>, String> {
    let Some(text) = parse_string_view(value, field)? else {
        return Ok(None);
    };
    Asset::new(text)
        .map(Some)
        .map_err(|err| format!("invalid {field}: {err}"))
}

pub(crate) fn import_instrument(value: &PitInstrument) -> Result<Option<Instrument>, String> {
    // `Instrument` owns its asset codes, so decoding a borrowed view
    // necessarily creates owned values here.
    let underlying = parse_asset_view(value.underlying_asset, "instrument.underlying_asset")?;
    let settlement = parse_asset_view(value.settlement_asset, "instrument.settlement_asset")?;

    match (underlying, settlement) {
        (Some(underlying), Some(settlement)) => Ok(Some(Instrument::new(underlying, settlement))),
        (None, None) => Ok(None),
        _ => Err(
            "instrument must provide both underlying_asset and settlement_asset or neither"
                .to_string(),
        ),
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
/// Trading instrument view.
///
/// The caller owns the memory referenced by both string views.
///
/// Semantics:
/// - both string views set: instrument is present;
/// - both string views not set: instrument is absent;
/// - only one string view set: invalid payload.
pub struct PitInstrument {
    /// Traded asset, for example `AAPL` or `BTC`.
    pub underlying_asset: PitStringView,
    /// Settlement asset, for example `USD`.
    pub settlement_asset: PitStringView,
}

#[cfg(test)]
mod tests {
    use super::{import_instrument, parse_asset_view, PitInstrument};
    use crate::PitStringView;

    #[test]
    fn import_instrument_accepts_complete_value() {
        let instrument = PitInstrument {
            underlying_asset: PitStringView {
                ptr: b"SPX".as_ptr(),
                len: 3,
            },
            settlement_asset: PitStringView {
                ptr: b"USD".as_ptr(),
                len: 3,
            },
        };

        let imported = import_instrument(&instrument)
            .expect("instrument must parse")
            .expect("instrument must be present");
        assert_eq!(imported.underlying_asset().as_ref(), "SPX");
        assert_eq!(imported.settlement_asset().as_ref(), "USD");
    }

    #[test]
    fn import_instrument_accepts_absent_value() {
        let instrument = PitInstrument::default();
        assert_eq!(import_instrument(&instrument).expect("must parse"), None);
    }

    #[test]
    fn import_instrument_rejects_partial_value() {
        let instrument = PitInstrument {
            underlying_asset: PitStringView {
                ptr: b"SPX".as_ptr(),
                len: 3,
            },
            settlement_asset: PitStringView::not_set(),
        };

        let err = import_instrument(&instrument).expect_err("partial instrument must fail");
        assert!(err.contains("both underlying_asset and settlement_asset"));
    }

    #[test]
    fn parse_asset_view_rejects_invalid_utf8() {
        let bytes = [0xff_u8, 0xfe_u8];
        let err = parse_asset_view(
            PitStringView {
                ptr: bytes.as_ptr(),
                len: bytes.len(),
            },
            "asset",
        )
        .expect_err("invalid utf-8 must fail");
        assert!(err.contains("not valid utf-8"));
    }
}
