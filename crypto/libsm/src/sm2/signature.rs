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

use super::ecc::*;
use super::field::FieldElem;
use num_bigint::BigUint;
use num_traits::*;
use sm3::hash::Sm3Hash;

use byteorder::{BigEndian, WriteBytesExt};
use sm2::error::Sm2Error;
use std::convert::TryFrom;
use pkcs8::der::asn1::Any;
use pkcs8::der;
use pkcs8::der::{Message, Encodable, Decodable};
use pkcs8::der::bigint::{UInt, U256};
use std::borrow::Borrow;

pub type Pubkey = Point;
pub type Seckey = BigUint;


pub struct Signature {
    r: BigUint,
    s: BigUint,
}

impl Signature {
    pub fn new(r_bytes: &[u8], s_bytes: &[u8]) -> Self {
        let r = BigUint::from_bytes_be(r_bytes);
        let s = BigUint::from_bytes_be(s_bytes);
        Signature { r, s }
    }
    /**
    导入der格式的签名
    */
    pub fn der_decode(buf: &[u8]) -> Result<Signature, der::Error> {
        Signature::try_from(Any::from_der(&buf)?)
    }
    /**
    导出der格式的签名
    */
    pub fn der_encode(&self) -> Result<Vec<u8>,der::Error> {
        self.to_vec()
    }

    #[inline]
    pub fn get_r(&self) -> &BigUint {
        &self.r
    }

    #[inline]
    pub fn get_s(&self) -> &BigUint {
        &self.s
    }
}

impl TryFrom<Any<'_>> for Signature {
    type Error = der::Error;

    fn try_from(value: Any<'_>) -> Result<Self, Self::Error> {
        value.sequence(|decoder| {

            let r = BigUint::from_bytes_be(decoder.uint_bytes().unwrap().as_bytes());

            let s = BigUint::from_bytes_be(decoder.uint_bytes().unwrap().as_bytes());

            Ok(Signature {
                r,
                s
            })
        })
    }
}

impl Message<'_> for Signature {
    fn fields<F, T>(&self, f: F) -> pkcs8::der::Result<T> where F: FnOnce(&[&dyn Encodable]) -> pkcs8::der::Result<T> {
        f(
            &[&UInt::<4usize>::from_be_slice(self.r.to_bytes_be().as_slice()),
                &UInt::<4usize>::from_be_slice(self.s.to_bytes_be().as_slice()), ]
        )
    }
}

pub struct SigCtx {
    curve: EccCtx,
}

impl SigCtx {
    pub fn new() -> SigCtx {
        SigCtx {
            curve: EccCtx::new(),
        }
    }

    pub fn get_curve_ctx(&self) -> &EccCtx{
        &self.curve
    }

    pub fn hash(&self, id: &str, pk: &Point, msg: &[u8]) -> [u8; 32] {
        let curve = &self.curve;

        let mut prepend: Vec<u8> = Vec::new();
        if id.len() * 8 > 65535 {
            panic!("ID is too long.");
        }
        prepend
            .write_u16::<BigEndian>((id.len() * 8) as u16)
            .unwrap();
        for c in id.bytes() {
            prepend.push(c);
        }

        let mut a = curve.get_a().to_bytes();
        let mut b = curve.get_b().to_bytes();

        prepend.append(&mut a);
        prepend.append(&mut b);

        let (x_g, y_g) = curve.to_affine(&curve.generator());
        let (mut x_g, mut y_g) = (x_g.to_bytes(), y_g.to_bytes());
        prepend.append(&mut x_g);
        prepend.append(&mut y_g);

        let (x_a, y_a) = curve.to_affine(pk);
        let (mut x_a, mut y_a) = (x_a.to_bytes(), y_a.to_bytes());
        prepend.append(&mut x_a);
        prepend.append(&mut y_a);

        let mut hasher = Sm3Hash::new(&prepend[..]);
        let z_a = hasher.get_hash();

        // Z_A = HASH_256(ID_LEN || ID || x_G || y_G || x_A || y_A)

        // e = HASH_256(Z_A || M)

        let mut prepended_msg: Vec<u8> = Vec::new();
        prepended_msg.extend_from_slice(&z_a[..]);
        prepended_msg.extend_from_slice(msg);

        let mut hasher = Sm3Hash::new(&prepended_msg[..]);
        hasher.get_hash()
    }

    pub fn recid_combine(&self, id: &str, pk: &Point, msg: &[u8]) -> Vec<u8> {
        let curve = &self.curve;

        let mut prepend: Vec<u8> = Vec::new();
        if id.len() * 8 > 65535 {
            panic!("ID is too long.");
        }
        prepend
            .write_u16::<BigEndian>((id.len() * 8) as u16)
            .unwrap();
        for c in id.bytes() {
            prepend.push(c);
        }

        let mut a = curve.get_a().to_bytes();
        let mut b = curve.get_b().to_bytes();

        prepend.append(&mut a);
        prepend.append(&mut b);

        let (x_g, y_g) = curve.to_affine(&curve.generator());
        let (mut x_g, mut y_g) = (x_g.to_bytes(), y_g.to_bytes());
        prepend.append(&mut x_g);
        prepend.append(&mut y_g);

        let (x_a, y_a) = curve.to_affine(pk);
        let (mut x_a, mut y_a) = (x_a.to_bytes(), y_a.to_bytes());
        prepend.append(&mut x_a);
        prepend.append(&mut y_a);

        let mut hasher = Sm3Hash::new(&prepend[..]);
        let z_a = hasher.get_hash();

        // Z_A = HASH_256(ID_LEN || ID || x_G || y_G || x_A || y_A)

        // e = HASH_256(Z_A || M)

        let mut prepended_msg: Vec<u8> = Vec::new();
        prepended_msg.extend_from_slice(&z_a[..]);
        prepended_msg.extend_from_slice(msg);

        prepended_msg
    }

    pub fn sign(&self, msg: &[u8], sk: &BigUint, pk: &Point) -> Signature {
        // Get the value "e", which is the hash of message and ID, EC parameters and public key
        let digest = self.hash("1234567812345678", pk, msg);

        self.sign_raw(&digest[..], sk)
    }

    pub fn sign_raw(&self, digest: &[u8], sk: &BigUint) -> Signature {
        let curve = &self.curve;
        // Get the value "e", which is the hash of message and ID, EC parameters and public key

        let e = BigUint::from_bytes_be(digest);

        // two while loops
        loop {
            // k = rand()
            // (x_1, y_1) = g^kg
            let k = self.curve.random_uint();

            let p_1 = curve.g_mul(&k);
            let (x_1, _) = curve.to_affine(&p_1);
            let x_1 = x_1.to_biguint();

            // r = e + x_1
            let r = (&e + x_1) % curve.get_n();
            if r == BigUint::zero() || &r + &k == *curve.get_n() {
                continue;
            }

            // s = (1 + sk)^-1 * (k - r * sk)
            let s1 = curve.inv_n(&(sk + BigUint::one()));

            let mut s2_1 = &r * sk;
            if s2_1 < k {
                s2_1 += curve.get_n();
            }
            let mut s2 = s2_1 - k;
            s2 %= curve.get_n();
            let s2 = curve.get_n() - s2;

            let s = (s1 * s2) % curve.get_n();

            if s != BigUint::zero() {
                // Output the signature (r, s)
                return Signature { r, s };
            }
            panic!("cannot sign")
        }
    }

    pub fn verify(&self, msg: &[u8], pk: &Point, sig: &Signature) -> bool {
        //Get hash value
        let digest = self.hash("1234567812345678", pk, msg);
        //println!("digest: {:?}", digest);
        self.verify_raw(&digest[..], pk, sig)
    }

    pub fn verify_raw(&self, digest: &[u8], pk: &Point, sig: &Signature) -> bool {
        if digest.len() != 32 {
            panic!("the length of digest must be 32-bytes.");
        }
        let e = BigUint::from_bytes_be(digest);

        let curve = &self.curve;
        // check r and s
        if *sig.get_r() == BigUint::zero() || *sig.get_s() == BigUint::zero() {
            return false;
        }
        if *sig.get_r() >= *curve.get_n() || *sig.get_s() >= *curve.get_n() {
            return false;
        }

        // calculate R
        let t = (sig.get_s() + sig.get_r()) % curve.get_n();
        if t == BigUint::zero() {
            return false;
        }

        let p_1 = curve.add(&curve.g_mul(sig.get_s()), &curve.mul(&t, pk));
        let (x_1, _) = curve.to_affine(&p_1);
        let x_1 = x_1.to_biguint();

        let r_ = (e + x_1) % curve.get_n();

        // check R == r?
        r_ == *sig.get_r()
    }

    pub fn new_keypair(&self) -> (Point, BigUint) {
        let curve = &self.curve;
        let mut sk: BigUint = curve.random_uint();
        let mut pk: Point = curve.g_mul(&sk);

        loop {
            if !pk.is_zero() {
                break;
            }
            sk = curve.random_uint();
            pk = curve.g_mul(&sk);
        }

        (pk, sk)
    }

    pub fn pk_from_sk(&self, sk: &BigUint) -> Point {
        let curve = &self.curve;
        if *sk >= *curve.get_n() || *sk == BigUint::zero() {
            panic!("invalid seckey");
        }
        curve.mul(&sk, &curve.generator())
    }

    pub fn load_pubkey(&self, buf: &[u8]) -> Result<Point, Sm2Error> {
        self.curve.bytes_to_point(buf)
    }

    pub fn serialize_pubkey(&self, p: &Point, compress: bool) -> Vec<u8> {
        self.curve.point_to_bytes(p, compress)
    }

    pub fn load_seckey(&self, buf: &[u8]) -> Result<BigUint, Sm2Error> {
        if buf.len() != 32 {
            return Err(Sm2Error::InvalidPrivate);
        }
        let sk = BigUint::from_bytes_be(buf);
        if sk > *self.curve.get_n() {
            Err(Sm2Error::InvalidPrivate)
        } else {
            Ok(sk)
        }
    }

    pub fn serialize_seckey(&self, x: &BigUint) -> Vec<u8> {
        if *x > *self.curve.get_n() {
            panic!("invalid secret key");
        }
        let x = FieldElem::from_biguint(x);
        x.to_bytes()
    }
}

impl Default for SigCtx {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use pkcs8::der::{Encodable, Decodable};

    #[test]
    fn test_sign_and_verify() {
        let string = String::from("abcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcd");
        let msg = string.as_bytes();

        let ctx = SigCtx::new();
        let (pk, sk) = ctx.new_keypair();
        let signature = ctx.sign(msg, &sk, &pk);

        assert!(ctx.verify(msg, &pk, &signature));
    }

    #[test]
    fn test_sig_encode_and_decode() {
        let string = String::from("abcdabcdabcdabcdabcdabcdabcdabcdabcdabcdabcd");
        let msg = string.as_bytes();

        let ctx = SigCtx::new();
        let (pk, sk) = ctx.new_keypair();

        let signature = ctx.sign(msg, &sk, &pk);
        let der = signature.der_encode().unwrap();
        let sig =  Signature::der_decode(&der).unwrap();
        assert!(ctx.verify(msg, &pk, &sig));
    }

    #[test]
    fn test_key_serialization() {
        let ctx = SigCtx::new();
        let (pk, sk) = ctx.new_keypair();

        let pk_v = ctx.serialize_pubkey(&pk, true);
        let new_pk = ctx.load_pubkey(&pk_v[..]).unwrap();
        assert!(ctx.curve.eq(&new_pk, &pk));

        let sk_v = ctx.serialize_seckey(&sk);
        let new_sk = ctx.load_seckey(&sk_v[..]).unwrap();
        assert_eq!(new_sk, sk);
    }

    #[test]
    fn test_gmssl() {
        let msg: &[u8] = &[
            0x66, 0xc7, 0xf0, 0xf4, 0x62, 0xee, 0xed, 0xd9, 0xd1, 0xf2, 0xd4, 0x6b, 0xdc, 0x10,
            0xe4, 0xe2, 0x41, 0x67, 0xc4, 0x87, 0x5c, 0xf2, 0xf7, 0xa2, 0x29, 0x7d, 0xa0, 0x2b,
            0x8f, 0x4b, 0xa8, 0xe0,
        ];

        let pk: &[u8] = &[
            4, 233, 185, 71, 125, 111, 174, 63, 105, 217, 19, 218, 72, 114, 185, 96, 243, 176, 1,
            8, 239, 132, 114, 119, 216, 38, 21, 117, 142, 223, 42, 157, 170, 123, 219, 65, 50, 238,
            191, 116, 238, 240, 197, 158, 1, 145, 177, 107, 112, 91, 101, 86, 50, 204, 218, 254,
            172, 2, 250, 33, 56, 176, 121, 16, 215,
        ];

        let sig: &[u8] = &[
            48, 69, 2, 33, 0, 171, 111, 172, 181, 242, 159, 198, 106, 33, 229, 104, 147, 245, 97,
            132, 141, 141, 17, 27, 97, 156, 159, 160, 188, 239, 78, 124, 17, 211, 124, 113, 26, 2,
            32, 53, 21, 4, 195, 198, 42, 71, 17, 110, 157, 113, 185, 178, 74, 147, 87, 129, 179,
            168, 163, 171, 126, 39, 156, 198, 29, 163, 199, 82, 25, 13, 112,
        ];

        let curve = EccCtx::new();
        let ctx = SigCtx::new();

        let pk = curve.bytes_to_point(&pk).unwrap();

        let sig =  Signature::der_decode(sig).unwrap();


        assert!(ctx.verify_raw(msg, &pk, &sig));
    }

    #[test]
    fn verify_third_test() {
        let ctx = SigCtx::new();
        let msg = "jonllen".to_string().into_bytes();

        let ecc_ctx = EccCtx::new();
        let pk_bz = hex::decode("044f954d8c4d7c0133e5f402c7e75623438c2dcee5ae5ee6c2f1fca51c60f7017e9cfad13514cd4e7faeca476a98eeb0b8a62c1f6add9794beead4a42291b94278").unwrap();
        let pk = ecc_ctx.bytes_to_point(&pk_bz).unwrap();

        let sig_bz = hex::decode("304402207e665a4d2781cb488bd374ccf1c8116e95ad0731c99e1dc36c189fd4daf0cb0202206a7ddd6483db176192b25aba9a92bc4de8b76e2c6d1559965ad06224d0725531").unwrap();
        let sig =  Signature::der_decode(&sig_bz).unwrap();

        assert!(ctx.verify(&msg, &pk, &sig));
    }

    fn test_verify_with_defined_key(msg_file: &str, pk: &str, sig_file: &str) {
        let ctx = SigCtx::new();
        let msg = read_file(msg_file);

        let sig = Signature::der_decode(&read_file(sig_file))
            .unwrap();
        let pk_raw = hex::decode(pk).unwrap();
        let pk = ctx.load_pubkey(&pk_raw).unwrap();
        let mut hasher = Sm3Hash::new(&msg[..]);
        let hash = hasher.get_hash();
        assert!(ctx.verify_raw(&hash, &pk, &sig))
    }

    #[test]
    fn test_comp_with_gm_ssl_verify() {
        test_sign_with_defined_key("README.md", "README.md.sig1",
                                   "045edb33db803919213f9694d1c65f6da6d2ccc67960059ac633db9e2ccf83ea77c683d9264f971cd4754dd64b103846b900351018843e852117bd07160bd9a543",
                                   "79c83d9bb4de39637721e4838a5b4201c71a405b1c0e708ed8ac2138548fba12");
    }

    #[test]
    fn test_comp_with_open_ssl_verity() {
        test_sign_with_defined_key("README.md", "README.md.sig2",
                                   "0489ea2aaddacd5ef0ce210df08731f3f0c5e5a6265b9be9958da5ebff4102860e7dae90a8e848bd0f6d2c34142ca4c78c9f9393be53b30d254aa7fffdb7b6a235",
                                   "b65278b14652f749d2f4bb36e5d43c722ccfa93b823ecfef67a4d709671bf7fe");
    }

    fn test_sign_with_defined_key(input_file_name: &str, output_file_name: &str,
                                  pk: &str, sk: &str) {
        let msg = read_file(input_file_name);
        //let msg = string.as_bytes();
        let pk = hex::decode(pk)
            .unwrap();
        let sk = hex::decode(sk)
            .unwrap();

        let ctx = SigCtx::new();

        let pk = ctx.load_pubkey(&pk).unwrap();
        let sk = ctx.load_seckey(&sk).unwrap();


        let signature = ctx.sign(&msg, &sk, &pk);

        write_file(output_file_name, signature.der_encode().unwrap().as_slice());
    }

    fn read_file(file_name: &str) -> Vec<u8> {
        let mut file = std::fs::File::open(file_name).unwrap();
        let mut contents = Vec::new();
        //file.read_to_string(&mut contents).unwrap();
        file.read_to_end(&mut contents);
        contents
    }

    fn write_file(file_name: &str, content: &[u8]) {
        let mut file = std::fs::File::create(file_name).expect("create failed");
        file.write_all(content).expect("write failed");
    }
}
