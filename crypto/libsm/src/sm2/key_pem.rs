use pkcs8::{ObjectIdentifier, AlgorithmIdentifier, PrivateKeyDocument, Version, PublicKeyDocument, Error};
use pkcs8::der;
use pkcs8::der::{Decodable};
use sm2::signature::{SigCtx, Seckey, Pubkey};
use pkcs8::der::{Message, Encodable, TagNumber};
use std::convert::TryFrom;
use pkcs8::der::asn1::{Any, BitString, OctetString, ContextSpecific};
use sm2::ecc::{EccCtx, Point};
use hex_literal::hex;
use rand::{OsRng, thread_rng, Rng, ThreadRng};
use num_bigint::BigUint;
use pkcs8::pkcs5::pbes2::Parameters;
use std::thread::Thread;

// type ecPrivateKey struct {
//     Version       int
//     PrivateKey    []byte
//     NamedCurveOID asn1.ObjectIdentifier `asn1:"optional,explicit,tag:0"`
//     PublicKey     asn1.BitString        `asn1:"optional,explicit,tag:1"`
// }
//SM2私钥格式定义 见SM2密码算法加密签名消息语法规范 附录 A.3
pub struct SM2Seckey {
    version: Version,
    private_key: Vec<u8>,
    curve_oid: Option<ObjectIdentifier>,
    public_key: Option<Vec<u8>>,
}

const SM2_NAMED_CURVE_TAG: TagNumber = TagNumber::new(0);
const PUBLIC_KEY_TAG: TagNumber = TagNumber::new(1);
const SM2_CURVE_OID: &'static str = "1.2.156.10197.1.301";
const SIG_ALGO_OID: &'static str = "1.2.840.10045.2.1";

impl SM2Seckey {
    fn new(ctx: &SigCtx, skey: &Seckey, oid: ObjectIdentifier) -> Self {
        let pkey = ctx.pk_from_sk(skey);
        let ecc_ctx = ctx.get_curve_ctx();

        let (mut pub_raw_x, mut pub_raw_y) = ecc_ctx.to_affine(&pkey);

        let mut pub_key_raw = Vec::<u8>::new();
        pub_key_raw.insert(0, 4);
        pub_key_raw.append(&mut pub_raw_x.to_bytes());
        pub_key_raw.append(&mut pub_raw_y.to_bytes());

        let sec_key_raw = skey.to_bytes_be();

        return SM2Seckey {
            version: Version::V1,
            private_key: sec_key_raw,
            curve_oid: Some(oid),
            public_key: Some(pub_key_raw),
        };
    }
}

impl TryFrom<Any<'_>> for SM2Seckey {
    type Error = der::Error;

    fn try_from(value: Any<'_>) -> Result<Self, Self::Error> {
        value.sequence(|decoder| {
            let version = Version::decode(decoder)?;

            let pri_key = decoder.octet_string()?.as_bytes().to_vec();

            let curv_oid = decoder.
                context_specific(SM2_NAMED_CURVE_TAG)?.map(|any| any.oid()).transpose()?;

            let pub_key = decoder.
                context_specific(PUBLIC_KEY_TAG)?.
                map(|any| any.bit_string()).transpose()?.
                map(|bit_string| { bit_string.as_bytes().to_vec() });


            Ok(SM2Seckey {
                version: version,
                private_key: pri_key,
                curve_oid: curv_oid,
                public_key: pub_key,
            })
        })
    }
}

impl Message<'_> for SM2Seckey {
    fn fields<F, T>(&self, f: F) -> pkcs8::der::Result<T> where F: FnOnce(&[&dyn Encodable]) -> pkcs8::der::Result<T> {
        f(
            &[&<u8 as From<Version>>::from(self.version),
                &OctetString::new(self.private_key.as_slice())?,
                &self.curve_oid.as_ref().map(|value| ContextSpecific {
                    tag_number: SM2_NAMED_CURVE_TAG,
                    value: value.into(),
                }),
                &BitString::new(self.public_key.as_ref().unwrap()).map(|value| ContextSpecific {
                    tag_number: PUBLIC_KEY_TAG,
                    value: value.into(),
                })?, ]
        )
    }
}
/**
以pem格式导出公钥
*/
pub fn pub_key_to_pem(ctx: &SigCtx, pk: &Pubkey) -> pkcs8::PublicKeyDocument {
    let sm2_curve_oid: ObjectIdentifier = SM2_CURVE_OID.parse().unwrap();
    let sig_algo_oid: ObjectIdentifier = SIG_ALGO_OID.parse().unwrap();

    let pub_key_raw = ctx.serialize_pubkey(pk,false);

    let sm2_pk_algo_id = AlgorithmIdentifier {
        oid: sig_algo_oid,
        parameters: Some(Any::from(&sm2_curve_oid)),
    };

    let sm2_pk = pkcs8::SubjectPublicKeyInfo {
        algorithm: sm2_pk_algo_id,
        subject_public_key: &pub_key_raw,
    };

    sm2_pk.into()
}
/**
导入pem格式的公钥
 */
pub fn pem_to_pub_key(pub_key: &pkcs8::PublicKeyDocument) -> Result<Pubkey, Error> {
    let pub_key_info = pub_key.spki();
    let pub_key_raw = pub_key_info.subject_public_key;
    let ecc_ctx = EccCtx::new();
    ecc_ctx.bytes_to_point(pub_key_raw).map_err(|_|{
        Error::KeyMalformed
    })
}
/**
以pem格式导出加密的私钥（PKCS#8）
 */
pub fn priv_key_to_encrypted_pem(ctx: &SigCtx, sk: &Seckey, pwd: &[u8]) -> Result<pkcs8::EncryptedPrivateKeyDocument,Error> {
    let sm2_curve_oid: ObjectIdentifier = SM2_CURVE_OID.parse().unwrap();
    let sig_algo_oid: ObjectIdentifier = SIG_ALGO_OID.parse().unwrap();

    let skey = SM2Seckey::new(ctx, sk, sm2_curve_oid);
    let skey_raw = skey.to_vec().unwrap();

    let sm2_sk_algo_id = AlgorithmIdentifier {
        oid: sig_algo_oid,
        parameters: Some(Any::from(&sm2_curve_oid)),
    };
    let sm2_sk = pkcs8::PrivateKeyInfo::new(sm2_sk_algo_id, &skey_raw);
    let sm2_document: PrivateKeyDocument = sm2_sk.into();
    let mut csprng = thread_rng();
    let salt = csprng.gen_range(u64::MIN, u64::MAX).to_be_bytes().to_vec();
    let iv_0 = csprng.gen_range(u64::MIN, u64::MAX);
    let iv_1 = csprng.gen_range(u64::MIN, u64::MAX);
    let iv: u128 = (((iv_1 as u128) << 64) + (iv_0 as u128)) as u128;
    let iv_bytes: [u8; 16] = iv.to_be_bytes();
    let enc_param = Parameters::pbkdf2_sha256_aes256cbc(2048,
                                                        &salt,
                                                        &iv_bytes).unwrap();
    sm2_document.encrypt_with_params(enc_param, pwd)
}
/**
导入pem格式的加密私钥（PKCS#8），返回解密后的私钥
 */
pub fn encrypted_pem_to_priv_key(enc_priv_key: &pkcs8::EncryptedPrivateKeyDocument, pwd: &[u8]) -> Result<Seckey,Error> {
    let priv_key_doc = enc_priv_key.decrypt(pwd)?;
    let priv_key_info = priv_key_doc.private_key_info();
    let priv_key_raw = priv_key_info.private_key;
    let sm2_key = SM2Seckey::try_from(Any::from_der(priv_key_raw)?)?;

    let sig_ctx = SigCtx::new();

    sig_ctx.load_seckey(&sm2_key.private_key).map_err(|_|{
        Error::KeyMalformed
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pkcs8::EncryptedPrivateKeyDocument;

    const PASSWORD: &[u8] = b"abc123";

    #[test]
    fn test_derive_pub_key() {
        let ctx = SigCtx::new();
        let (pk, _) = ctx.new_keypair();
        let pub_key_pem = pub_key_to_pem(&ctx,&pk);
        //print!("{:?}",priv_key_encrypted_pem.to_pem().to_string());
        pub_key_pem.write_pem_file("sm2_pub_document.pem");
    }

    #[test]
    fn test_input_pub_key() {
        let pub_key = PublicKeyDocument::read_pem_file("sm2_pub_document.pem").unwrap();
        let sm2_pub_key = pem_to_pub_key(&pub_key).unwrap();
        println!("{:?}", sm2_pub_key.to_string())
    }

    #[test]
    fn test_derive_priv_key() {
        let ctx = SigCtx::new();
        let (_, sk) = ctx.new_keypair();
        let priv_key_encrypted_pem = priv_key_to_encrypted_pem(&ctx, &sk, PASSWORD).unwrap();
        //print!("{:?}",priv_key_encrypted_pem.to_pem().to_string());
        priv_key_encrypted_pem.write_pem_file("sm2_enc_document.pem");
    }

    #[test]
    fn test_input_priv_key() {
        let enc_priv_key = EncryptedPrivateKeyDocument::read_pem_file("sm2_enc_document.pem").unwrap();
        let sm2_priv_key = encrypted_pem_to_priv_key(&enc_priv_key, PASSWORD).unwrap();
        println!("{:?}", sm2_priv_key.to_bytes_be())
    }
}
