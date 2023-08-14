// Copyright 2018 Cryptape Technology LLC.
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
// #![cfg_attr(feature = "internal_benches", allow(unstable_features), feature(test))]

pub mod sm2;
pub mod sm3;
pub mod sm4;

extern crate byteorder;
extern crate rand;

extern crate num_bigint;
extern crate num_integer;
extern crate num_traits;

extern crate pkcs8;
extern crate hex_literal;
extern crate hex;
extern crate thiserror;
#[macro_use]
extern crate lazy_static;
