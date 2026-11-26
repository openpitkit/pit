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

fn unchecked_option(options: &JsValue) -> Result<JsValue, JsValue> {
    // ruleid: js-option-unknown-keys
    read_field(options, "mode")
}

fn unchecked_reflect(options: &JsValue) -> Result<JsValue, JsValue> {
    // ruleid: js-option-unknown-keys
    js_sys::Reflect::get(options, &JsValue::from_str("name"))
}

fn unchecked_helper(options: JsValue) -> Result<(), JsValue> {
    // ruleid: js-option-unknown-keys
    optional_wrapper_iter_field::<Barrier>(&options, "brokerBarriers")?;
    Ok(())
}

fn unchecked_optional_fields(options: &JsValue) -> Result<(), JsValue> {
    // ruleid: js-option-unknown-keys
    read_optional_string(options, "name")?;
    // ruleid: js-option-unknown-keys
    read_optional_bool(options, "enabled")?;
    Ok(())
}

fn checked_option(options: &JsValue) -> Result<JsValue, JsValue> {
    reject_unknown_keys(options, "options", &["mode"])?;
    // ok: js-option-unknown-keys
    read_field(options, "mode")
}

fn checked_configure(options: JsValue) -> Result<(), JsValue> {
    require_object(&options, "options", &["brokerBarriers"])?;
    // ok: js-option-unknown-keys
    optional_wrapper_iter_field::<Barrier>(&options, "brokerBarriers")?;
    Ok(())
}

fn checked_instance(options: &JsValue) -> Result<JsValue, JsValue> {
    reject_unknown_keys_with(options, "instance", &["name"], invalid)?;
    // ok: js-option-unknown-keys
    Reflect::get(options, &JsValue::from_str("name"))
}

fn checked_other_object(options: &JsValue, other: &JsValue) -> Result<JsValue, JsValue> {
    reject_unknown_keys(other, "other", &["mode"])?;
    // ruleid: js-option-unknown-keys
    read_field(options, "mode")
}

fn dynamic_key(value: &JsValue, key: &str) -> Result<JsValue, JsValue> {
    // ok: js-option-unknown-keys
    Reflect::get(value, &JsValue::from_str(key))
}

fn reject_thenable(value: &JsValue) -> Result<(), JsValue> {
    // ok: js-option-unknown-keys
    let then = Reflect::get(value, &JsValue::from_str("then"))?;
    Ok(())
}
