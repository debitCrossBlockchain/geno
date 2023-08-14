use crate::rand::rand;
use crate::signing::bytes_to_hex_str;
use crate::signing::hex_str_to_bytes;
use crate::signing::Context;
use crate::signing::Error;
use crate::signing::PrivateKey;
use crate::signing::PublicKey;
use crate::HashInstanceRef;
use libsm::sm2::error::Sm2Error;
use libsm::sm2::key_pem::SM2Seckey;
use libsm::sm2::signature::*;
use libsm::sm2::signature::{Pubkey, Seckey, Signature};
use std::convert::TryFrom;
// use num_bigint::BigUint;
#[derive(Clone)]
pub struct Sm2PrivateKey {
    private: Vec<u8>,
}

impl Sm2PrivateKey {
    pub fn from_hex(s: &str) -> Result<Self, Error> {
        hex_str_to_bytes(s).map(|key_bytes| Sm2PrivateKey { private: key_bytes })
    }

    pub fn from_bytes(v: &[u8]) -> Result<Self, Error> {
        Ok(Sm2PrivateKey {
            private: Vec::from(v.clone()),
        })
    }
}

impl From<Sm2Error> for Error {
    fn from(e: Sm2Error) -> Self {
        Error::ParseError("test".to_string())
    }
}

impl Sm2PrivateKey {
    pub fn new() -> Self {
        let ctx = SigCtx::new();
        let (_, sk) = ctx.new_keypair();
        let sk_v = ctx.serialize_seckey(&sk);
        Self { private: sk_v }
        // let ctx = SigCtx::new();
        // let (pk, sk) = ctx.new_keypair();
        // let result = Sm2PrivateKey::from_hex(
        //     bytes_to_hex_str(&*ctx.serialize_seckey(&sk )).as_str(),
        // ).unwrap();
        // result
    }
}

impl PrivateKey for Sm2PrivateKey {
    fn get_algorithm_name(&self) -> &str {
        "sm2"
    }

    fn as_hex(&self) -> String {
        bytes_to_hex_str(&self.private)
    }

    fn as_slice(&self) -> &[u8] {
        &self.private
    }
}

#[derive(Clone)]
pub struct Sm2PublicKey {
    public: Vec<u8>,
}

impl Sm2PublicKey {
    pub fn from_hex(s: &str) -> Result<Self, Error> {
        hex_str_to_bytes(s).map(|key_bytes| Sm2PublicKey { public: key_bytes })
    }

    pub fn from_bytes(v: &[u8]) -> Result<Self, Error> {
        Ok(Sm2PublicKey {
            public: Vec::from(v.clone()),
        })
    }
}

impl PublicKey for Sm2PublicKey {
    fn get_algorithm_name(&self) -> &str {
        "sm2"
    }

    fn as_hex(&self) -> String {
        bytes_to_hex_str(&self.public)
    }

    fn as_slice(&self) -> &[u8] {
        &self.public
    }
}

pub struct Sm2Context {}

impl Sm2Context {
    pub fn new() -> Self {
        Sm2Context {}
    }
}

impl Default for Sm2Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context for Sm2Context {
    fn get_algorithm_name(&self) -> &str {
        "sm2"
    }

    fn sign(&self, message: &[u8], key: &dyn PrivateKey) -> Result<String, Error> {
        let ctx = SigCtx::new();
        let sk = ctx.load_seckey(key.as_slice())?;
        let pk = ctx.pk_from_sk(&sk);

        let signature = ctx.sign(message, &sk, &pk);
        let result = signature.der_encode();
        let re = result.clone().unwrap();
        let ree = bytes_to_hex_str(&*re);
        match result {
            Ok(s) => Ok(bytes_to_hex_str(&*s)),
            Err(err) => Err(Error::ParseError("test sign".to_string())),
        }
    }
    // fn verify(&self, signature: &[u8], message: &[u8], key: &dyn PublicKey) -> Result<bool, Error> {
    //     let ctx = SigCtx::new();
    //     let pk = ctx.load_pubkey(&key.as_slice()[..])?;
    //     let mstr= std::str::from_utf8(signature).unwrap();
    //     let sig_bz = hex::decode(mstr.to_string()).unwrap();
    //     let sig = Signature::der_decode(&*sig_bz).unwrap();
    //     // let sig_bz = bytes_to_hex_str(signature).into_bytes();
    //     // let mes= std::str::from_utf8(message).unwrap();
    //     // println!("signature {:}",mstr.to_string());
    //     // println!("message {:}",mes.to_string());
    //     // let sig = Signature::der_decode(&*signature).unwrap();
    //
    //     // let s = hex::encode(signature);
    //
    //     // let sig = Signature::der_decode(&*sig_bz).unwrap();
    //
    //     // let m = hex::encode(message);
    //     // let mess = hex::decode(m).unwrap();
    //     // let m = hex::encode(message);
    //     //
    //     let result = ctx.verify(message, &pk, &sig);
    //
    //     Ok(result)
    //
    //
    // }
    fn verify(&self, signature: &[u8], message: &[u8], key: &dyn PublicKey) -> Result<bool, Error> {
        let ctx = SigCtx::new();
        let pk = ctx.load_pubkey(&key.as_slice()[..])?;
        // let mes = HashInstanceRef.hash(message);
        let sig = Signature::der_decode(signature);
        if sig.is_err() {
            return Err(Error::ParseError("invalid signature".to_string()));
        }
        // let result = ctx.verify(mes.as_slice(), &pk, &sig);
        let result = ctx.verify(message, &pk, &sig.unwrap());

        Ok(result)
    }

    fn get_public_key(&self, private_key: &dyn PrivateKey) -> Result<Box<dyn PublicKey>, Error> {
        let ctx = SigCtx::new();
        let sk = ctx.load_seckey(private_key.as_slice())?;
        let pk = ctx.pk_from_sk(&sk);

        let result = Sm2PublicKey::from_bytes(&*ctx.serialize_pubkey(&pk, true));
        match result {
            Err(err) => Err(err),
            Ok(pkk) => Ok(Box::new(pkk)),
        }
    }

    fn new_random_private_key(&self) -> Result<Box<dyn PrivateKey>, Error> {
        // let ctx = SigCtx::new();
        // let (_, sk) = ctx.new_keypair();
        // let sk_v = ctx.serialize_seckey(&sk );
        // let pri = Sm2PrivateKey { private: sk_v };
        // Ok(Box::new(pri))
        Ok(Box::new(Sm2PrivateKey::new()))
    }
}

#[cfg(test)]
mod sm2_test {
    use super::super::create_context;
    use super::super::CryptoFactory;
    use super::super::PrivateKey;
    use super::super::PublicKey;
    use super::super::Signer;
    use super::Sm2PrivateKey;
    use super::Sm2PublicKey;
    use crate::hex_str_to_bytes;
    use crate::signing::sm2::Sm2Context;
    use crate::signing::Context;
    use libsm::sm2::key_pem::SM2Seckey;

    static KEY1_PRIV_HEX: &'static str =
        "d20bbd475328319531d997f638c2333b8960e2193e5d80343d372c7ee1f1fd39";
    static KEY1_PUB_HEX: &'static str =
        "0382bb996448006d768439de4598b9222280a53f27cbba232ec4751cf4db39e0f8";
    static MSG1: &'static str = "test";
    static MSG1_KEY1_SIG: &'static str = "304502205ff9f875f13926c57d1ceafc59f2ee19ce0d19c10281832783e42ffef767295502210097cd8afa8fbd723160eecdba0705b7c387b61e4c553d89c86b32690db7f6161a";

    static KEY2_PRIV_HEX: &'static str =
        "cc8e94e71a230fd0ef7dcc6b925ee90e141ecf8b93e2c16aa81a1c67de1f80e8";
    static KEY2_PUB_HEX: &'static str =
        "037f879c2d04a3867fad5f67ec867654a01c842273cb0f1332a62fa6ee762e081e";
    static MSG2: &'static str = "test2";
    static MSG2_KEY2_SIG: &'static str = "30450221008b8982d017bdc0c05a1363fb3b969d0185a24a20b0279f4b1d896dd72ae5067002201af6064318087d5ce2c131af4c7c54a02a09e4a75ee730bdee0ceed30a9ec0a0";
    #[test]
    fn test() {
        use crate::signing::bytes_to_hex_str;
        use crate::signing::hex_str_to_bytes;
        use libsm::sm2::signature::{Pubkey, Seckey, SigCtx, Signature};

        let ctx = create_context("sm2").unwrap();

        let pri2 = ctx.new_random_private_key().unwrap();

        let mess = MSG1;

        let sign_ret = ctx.sign(mess.as_bytes(), &*pri2).unwrap();
        // let resign = Vec::from(
        //     hex_str_to_bytes(sign_ret.as_str()).unwrap(),
        // );
        let resign = hex_str_to_bytes(sign_ret.as_str()).unwrap();
        let pubkey = ctx.get_public_key(&*pri2).unwrap();
        let b = ctx
            .verify(&*resign, mess.as_bytes(), pubkey.as_ref())
            .unwrap();
        println!("{}", b)
    }

    #[test]
    fn test2() {
        use crate::signing::bytes_to_hex_str;
        use crate::signing::hex_str_to_bytes;
        use libsm::sm2::signature::{Pubkey, Seckey, SigCtx, Signature};

        let ctx = create_context("eddsa_ed25519").unwrap();

        let pri2 = ctx.new_random_private_key().unwrap();
        // println!("{}",pri2.get_address());
        let factory = CryptoFactory::new(&*ctx);
        let signer = factory.new_signer(&*pri2);
        let signature1 = signer.sign(&String::from(MSG1).into_bytes()).unwrap();
        let resign = Vec::from(hex_str_to_bytes(signature1.as_str()).unwrap());
        // let resign =
        //     hex_str_to_bytes(signature1.as_str()).unwrap();
        let pubkey = ctx.get_public_key(&*pri2).unwrap();
        let pub_key = Sm2PublicKey::from_hex(bytes_to_hex_str(pubkey.as_slice()).as_str());
        // println!("{}",pub_key.unwrap().get_address());
        let b = ctx
            .verify(&resign, &String::from(MSG1).into_bytes(), &*pubkey)
            .unwrap();
        println!("{}", b)
    }

    #[test]
    fn generate_key() {
        use crate::signing::bytes_to_hex_str;
        use crate::signing::hex_str_to_bytes;
        use libsm::sm2::signature::{Pubkey, Seckey, SigCtx, Signature};

        let ctx = SigCtx::new();
        let context = create_context("sm2").unwrap();
        let (pk, sk) = ctx.new_keypair();
        let result =
            Sm2PrivateKey::from_hex(bytes_to_hex_str(&*ctx.serialize_seckey(&sk)).as_str())
                .unwrap();

        let s = ctx.serialize_seckey(&sk);
        println!("priv address:{:?}", result.get_address());
        println!("priv hex:{:?}", result.as_hex());
        let result1 =
            Sm2PublicKey::from_hex(bytes_to_hex_str(&*ctx.serialize_pubkey(&pk, true)).as_str())
                .unwrap();
        println!("pub address:{:?}", result1.get_address());
        println!("pub hex:{:?}", result1.as_hex());

        let factory = CryptoFactory::new(&*context);
        let signer = factory.new_signer(&result);
        let signature1 = signer.sign(&String::from(MSG1).into_bytes()).unwrap();
        let signature2 = signer.sign(&String::from(MSG2).into_bytes()).unwrap();
        println!("signature test 1 hex:{:?}", signature1);
        println!("signature test 2 hex:{:?}", signature2);

        let ctx1 = SigCtx::new();
        let signature = ctx1.sign(&String::from(MSG1).into_bytes(), &sk, &pk);
        let signature2 = ctx1.sign(&String::from(MSG1).into_bytes(), &sk, &pk);
        let signature3 = ctx1.sign(&String::from(MSG1).into_bytes(), &sk, &pk);

        let result = signature.der_encode();
        let re = result.clone().unwrap();
        let ree = bytes_to_hex_str(&*re);

        println!("ree : {:}", ree);
        let a = ctx.verify(&String::from(MSG1).into_bytes(), &pk, &signature);
        let b = ctx1.verify(&String::from(MSG1).into_bytes(), &pk, &signature2);
        let c = ctx1.verify(&String::from(MSG1).into_bytes(), &pk, &signature3);
        println!("verifya:{:?}", a);
        println!("verifyb:{:?}", b);
        println!("verifyv:{:?}", c);
        assert!(ctx1.verify(&String::from(MSG1).into_bytes(), &pk, &signature));
    }
    #[test]
    fn sign() {
        use crate::signing::bytes_to_hex_str;
        use crate::signing::create_private_key;
        use crate::signing::hex_str_to_bytes;
        use libsm::sm2::signature::{Pubkey, Seckey, SigCtx, Signature};

        let ctx = Sm2Context::new();
        let ctx2 = Sm2Context::new();
        let private_key = create_private_key("sm2", KEY1_PRIV_HEX);
        let private_key_box = &*private_key.unwrap();
        println!(
            "private_key.get_pubkey():{:?}",
            private_key_box.get_pubkey()
        );
        println!("KEY1_PUB_HEX.to_string():{:?}", KEY1_PUB_HEX.to_string());
        let pubulic_key = Sm2PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        let raw_hash = String::from(MSG1).into_bytes();
        let sign_ret = ctx.sign(&raw_hash, private_key_box);
        let resign = Vec::from(hex_str_to_bytes(sign_ret.unwrap().as_str()).unwrap());
        let result = ctx2.verify(resign.as_slice(), raw_hash.as_slice(), &pubulic_key);

        assert_eq!(result.unwrap(), true);
    }
    #[test]
    fn sign1() {
        use crate::signing::bytes_to_hex_str;
        use crate::signing::create_private_key;
        use crate::signing::hex_str_to_bytes;
        use libsm::sm2::signature::{Pubkey, Seckey, SigCtx, Signature};

        let ctx = Sm2Context::new();
        let ctx2 = Sm2Context::new();
        let private_key = create_private_key("sm2", KEY1_PRIV_HEX);
        let private_key_box = &*private_key.unwrap();
        println!(
            "private_key.get_pubkey():{:?}",
            private_key_box.get_pubkey()
        );
        println!("KEY1_PUB_HEX.to_string():{:?}", KEY1_PUB_HEX.to_string());
        let pubulic_key = Sm2PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        let raw_hash = String::from(MSG1).into_bytes();
        let sign_ret = ctx.sign(&raw_hash, private_key_box);

        let resign = Vec::from(hex_str_to_bytes(&*sign_ret.unwrap()).unwrap());
        let result = ctx2.verify(resign.as_slice(), raw_hash.as_slice(), &pubulic_key);

        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn hex_key() {
        let priv_key = Sm2PrivateKey::from_hex(KEY1_PRIV_HEX).unwrap();
        println!("priv address:{:?}", priv_key.get_address());
        assert_eq!(priv_key.get_algorithm_name(), "sm2");
        assert_eq!(priv_key.as_hex(), KEY1_PRIV_HEX);

        let pub_key = Sm2PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        println!("pub_key address:{:?}", pub_key.get_address());
        assert_eq!(pub_key.get_algorithm_name(), "sm2");
        assert_eq!(pub_key.as_hex(), KEY1_PUB_HEX);

        let priv_key2 = Sm2PrivateKey::from_hex(KEY2_PRIV_HEX).unwrap();
        println!("priv2 address:{:?}", priv_key2.get_address());
        assert_eq!(priv_key2.get_algorithm_name(), "sm2");
        assert_eq!(priv_key2.as_hex(), KEY2_PRIV_HEX);

        let pub_key2 = Sm2PublicKey::from_hex(KEY2_PUB_HEX).unwrap();
        println!("pub_key2 address:{:?}", pub_key2.get_address());
        assert_eq!(pub_key2.get_algorithm_name(), "sm2");
        assert_eq!(pub_key2.as_hex(), KEY2_PUB_HEX);
    }

    #[test]
    fn priv_to_public_key() {
        let context = create_context("sm2").unwrap();
        assert_eq!(context.get_algorithm_name(), "sm2");

        let priv_key1 = Sm2PrivateKey::from_hex(KEY1_PRIV_HEX).unwrap();
        assert_eq!(priv_key1.get_algorithm_name(), "sm2");
        assert_eq!(priv_key1.as_hex(), KEY1_PRIV_HEX);

        let public_key1 = context.get_public_key(&priv_key1).unwrap();
        println!("public_key1.as_hex():{:?}", public_key1.as_hex());
        println!("KEY1_PUB_HEX():{:?}", KEY1_PUB_HEX);
        assert_eq!(public_key1.as_hex(), KEY1_PUB_HEX);

        let priv_key2 = Sm2PrivateKey::from_hex(KEY2_PRIV_HEX).unwrap();
        assert_eq!(priv_key2.get_algorithm_name(), "sm2");
        assert_eq!(priv_key2.as_hex(), KEY2_PRIV_HEX);

        let public_key2 = context.get_public_key(&priv_key2).unwrap();
        assert_eq!(public_key2.as_hex(), KEY2_PUB_HEX);
    }

    #[test]
    fn check_invalid_digit() {
        let mut priv_chars: Vec<char> = KEY1_PRIV_HEX.chars().collect();
        priv_chars[3] = 'i';
        let priv_result =
            Sm2PrivateKey::from_hex(priv_chars.into_iter().collect::<String>().as_str());
        assert!(priv_result.is_err());

        let mut pub_chars: Vec<char> = KEY1_PUB_HEX.chars().collect();
        pub_chars[3] = 'i';
        let result = Sm2PublicKey::from_hex(pub_chars.into_iter().collect::<String>().as_str());
        assert!(result.is_err());
    }

    #[test]
    fn verification1() {
        let context = create_context("sm2").unwrap();
        assert_eq!(context.get_algorithm_name(), "sm2");

        let pub_key1 = Sm2PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        assert_eq!(pub_key1.get_algorithm_name(), "sm2");
        assert_eq!(pub_key1.as_hex(), KEY1_PUB_HEX);
        let sign_byte = hex_str_to_bytes(MSG1_KEY1_SIG).unwrap();
        let result = context.verify(
            sign_byte.as_slice(),
            &String::from(MSG1).into_bytes(),
            &pub_key1,
        );
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn verification2() {
        let context = create_context("sm2").unwrap();
        assert_eq!(context.get_algorithm_name(), "sm2");

        let pub_key1 = Sm2PublicKey::from_hex(KEY2_PUB_HEX).unwrap();
        assert_eq!(pub_key1.get_algorithm_name(), "sm2");
        assert_eq!(pub_key1.as_hex(), KEY2_PUB_HEX);
        let sign_byte = hex_str_to_bytes(MSG2_KEY2_SIG).unwrap();
        let result = context.verify(
            sign_byte.as_slice(),
            &String::from(MSG2).into_bytes(),
            &pub_key1,
        );
        assert_eq!(result.unwrap(), true);
    }
    #[test]
    fn verification_error() {
        let context = create_context("sm2").unwrap();
        assert_eq!(context.get_algorithm_name(), "sm2");

        let pub_key1 = Sm2PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        assert_eq!(pub_key1.get_algorithm_name(), "sm2");
        assert_eq!(pub_key1.as_hex(), KEY1_PUB_HEX);
        let sign_byte = hex_str_to_bytes(MSG2_KEY2_SIG).unwrap();
        // This signature doesn't match for MSG1/KEY1
        let result = context.verify(
            sign_byte.as_slice(),
            &String::from(MSG1).into_bytes(),
            &pub_key1,
        );
        assert_eq!(result.unwrap(), false);
    }
}
