use crate::rand::rand;
use crate::signing::bytes_to_hex_str;
use crate::signing::hex_str_to_bytes;
use crate::signing::Context;
use crate::signing::Error;
use crate::signing::PrivateKey;
use crate::signing::PublicKey;
use ed25519_dalek::*;
use std::convert::TryFrom;
#[derive(Clone)]
pub struct EddsaEd25519PrivateKey {
    private: Vec<u8>,
}

impl EddsaEd25519PrivateKey {
    pub fn from_hex(s: &str) -> Result<Self, Error> {
        hex_str_to_bytes(s).map(|key_bytes| EddsaEd25519PrivateKey { private: key_bytes })
    }

    pub fn from_bytes(v: &[u8]) -> Result<Self, Error> {
        Ok(EddsaEd25519PrivateKey {
            private: Vec::from(v.clone()),
        })
    }
}

impl From<SignatureError> for Error {
    fn from(e: SignatureError) -> Self {
        Error::SigningError(Box::new(e))
    }
}

impl EddsaEd25519PrivateKey {
    pub fn new() -> Self {
        Self { private: rand() }
    }
}

impl PrivateKey for EddsaEd25519PrivateKey {
    fn get_algorithm_name(&self) -> &str {
        "eddsa_ed25519"
    }

    fn as_hex(&self) -> String {
        bytes_to_hex_str(&self.private)
    }

    fn as_slice(&self) -> &[u8] {
        &self.private
    }
}

#[derive(Clone)]
pub struct EddsaEd25519PublicKey {
    public: Vec<u8>,
}

impl EddsaEd25519PublicKey {
    pub fn from_hex(s: &str) -> Result<Self, Error> {
        hex_str_to_bytes(s).map(|key_bytes| EddsaEd25519PublicKey { public: key_bytes })
    }

    pub fn from_bytes(v: &[u8]) -> Result<Self, Error> {
        Ok(EddsaEd25519PublicKey {
            public: Vec::from(v.clone()),
        })
    }
}

impl PublicKey for EddsaEd25519PublicKey {
    fn get_algorithm_name(&self) -> &str {
        "eddsa_ed25519"
    }

    fn as_hex(&self) -> String {
        bytes_to_hex_str(&self.public)
    }

    fn as_slice(&self) -> &[u8] {
        &self.public
    }
}

pub struct EddsaEd25519Context {}

impl EddsaEd25519Context {
    pub fn new() -> Self {
        EddsaEd25519Context {}
    }
}

impl Default for EddsaEd25519Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Context for EddsaEd25519Context {
    fn get_algorithm_name(&self) -> &str {
        "eddsa_ed25519"
    }

    fn sign(&self, message: &[u8], key: &dyn PrivateKey) -> Result<String, Error> {
        let sk = ed25519_dalek::SecretKey::from_bytes(key.as_slice())?;
        let pk = ed25519_dalek::PublicKey::from(&sk);
        let expanded_sk = ExpandedSecretKey::from(&sk);
        let signature = expanded_sk.sign(message, &pk);
        Ok(signature
            .to_bytes()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(""))
    }

    fn verify(&self, signature: &[u8], message: &[u8], key: &dyn PublicKey) -> Result<bool, Error> {
        let pk = ed25519_dalek::PublicKey::from_bytes(key.as_slice())?;
        let sig = Signature::new(<[u8; 64]>::try_from(signature).unwrap());
        let result = pk.verify(message, &sig);

        match result {
            Ok(()) => Ok(true),
            Err(SignatureError) => Ok(false),
            Err(err) => Err(Error::from(err)),
        }
    }

    fn get_public_key(&self, private_key: &dyn PrivateKey) -> Result<Box<dyn PublicKey>, Error> {
        let sk = ed25519_dalek::SecretKey::from_bytes(private_key.as_slice())?;
        let result =
            EddsaEd25519PublicKey::from_bytes(ed25519_dalek::PublicKey::from(&sk).as_bytes());
        match result {
            Err(err) => Err(err),
            Ok(pk) => Ok(Box::new(pk)),
        }
    }

    fn new_random_private_key(&self) -> Result<Box<dyn PrivateKey>, Error> {
        Ok(Box::new(EddsaEd25519PrivateKey { private: rand() }))
    }
}

#[cfg(test)]
mod eddsa_ed25519_test {
    use super::super::create_context;
    use super::super::CryptoFactory;
    use super::super::PrivateKey;
    use super::super::PublicKey;
    use super::super::Signer;
    use super::EddsaEd25519PrivateKey;
    use super::EddsaEd25519PublicKey;
    use crate::signing::{create_private_key, verify_data};

    static KEY1_PRIV_HEX: &'static str =
        "2f1e7b7a130d7ba9da0068b3bb0ba1d79e7e77110302c9f746c3c2a63fe40088";
    static KEY1_PUB_HEX: &'static str =
        "38f0d4603bcdc3a88d169709f86600d4bda2dea98f0366f30779970f51815dfa";

    static KEY2_PRIV_HEX: &'static str =
        "51b845c2cdde22fe646148f0b51eaf5feec8c82ee921d5e0cbe7619f3bb9c62d";
    static KEY2_PUB_HEX: &'static str =
        "7e2830efd9e4fe85611f70ebb50ccf7f8432717b19296659b01f1898a752fc02";

    static MSG1: &'static str = "test";
    static MSG1_KEY1_SIG: &'static str = "30131ffee0beca6d41a08469797f4fcdff191272c79afdd11836ef983aa3d8e87670be98bbe9a839af80e4ab07f1af99424f25f9ec95471c76c3028bee66a907";

    static MSG2: &'static str = "test2";
    static MSG2_KEY2_SIG: &'static str = "ddd742cd73c2188f98b93c63b462fa4f6e61c37f52c9e6a74b39c359591a20068cf61718e9b954a36905f8eadb7220ba81f9ba76f3bdaa3cd3b54e0989daa00b";

    #[test]
    fn generate_key() {
        let ctx = create_context("eddsa_ed25519").unwrap();
        // let pri1 = ctx.new_random_private_key().unwrap();
        // let pub1 = ctx.get_public_key(&*pri1).unwrap();
        // println!("pri1.as_hex() {}",pri1.as_hex());println!("pri1.as_hex() {}",pri1.as_hex());
        // println!("pub1.as_hex() {}",pub1.as_hex());
        let p = create_private_key(
            "eddsa_ed25519",
            "2f1e7b7a130d7ba9da0068b3bb0ba1d79e7e77110302c9f746c3c2a63fe40088",
        )
        .unwrap();
        // let priv_key = EddsaEd25519PrivateKey::from_hex(KEY1_PRIV_HEX).unwrap();
        // println!("priv address:{:?}", priv_key.get_address());
        // assert_eq!(priv_key.get_algorithm_name(), "eddsa_ed25519");
        // assert_eq!(priv_key.as_hex(), KEY1_PRIV_HEX);
        //
        // let pub_key = EddsaEd25519PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        // println!("pub_key address:{:?}", pub_key.get_address());
        // assert_eq!(pub_key.get_algorithm_name(), "eddsa_ed25519");
        // assert_eq!(pub_key.as_hex(), KEY1_PUB_HEX);
        let pu = ctx.get_public_key(&*p).unwrap();
        println!("pri.as_hex() {}", p.as_hex());
        println!("pub1.as_hex() {}", pu.as_hex());
        let s = p.sign_data(MSG1.as_bytes(), "eddsa_ed25519");
        println!("s {}", s);
        let re = ctx.verify(s.as_bytes(), MSG1.as_bytes(), &*pu).unwrap();
        println!("re {}", re);
        let re1 = verify_data(&*s, MSG1.as_ref(), KEY1_PUB_HEX, "eddsa_ed25519");
        println!("re1 {}", re1);
        // let sign_ret = ctx.sign(MSG1.as_bytes(), priv_key.).unwrap();
    }

    #[test]
    fn hex_key() {
        let priv_key = EddsaEd25519PrivateKey::from_hex(KEY1_PRIV_HEX).unwrap();
        println!("priv address:{:?}", priv_key.get_address());
        assert_eq!(priv_key.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(priv_key.as_hex(), KEY1_PRIV_HEX);

        let pub_key = EddsaEd25519PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        println!("pub_key address:{:?}", pub_key.get_address());
        assert_eq!(pub_key.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(pub_key.as_hex(), KEY1_PUB_HEX);

        let priv_key2 = EddsaEd25519PrivateKey::from_hex(KEY2_PRIV_HEX).unwrap();
        println!("priv2 address:{:?}", priv_key2.get_address());
        assert_eq!(priv_key2.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(priv_key2.as_hex(), KEY2_PRIV_HEX);

        let pub_key2 = EddsaEd25519PublicKey::from_hex(KEY2_PUB_HEX).unwrap();
        println!("pub_key2 address:{:?}", pub_key2.get_address());
        assert_eq!(pub_key2.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(pub_key2.as_hex(), KEY2_PUB_HEX);
    }

    #[test]
    fn priv_to_public_key() {
        let context = create_context("eddsa_ed25519").unwrap();
        assert_eq!(context.get_algorithm_name(), "eddsa_ed25519");

        let priv_key1 = EddsaEd25519PrivateKey::from_hex(KEY1_PRIV_HEX).unwrap();
        assert_eq!(priv_key1.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(priv_key1.as_hex(), KEY1_PRIV_HEX);

        let public_key1 = context.get_public_key(&priv_key1).unwrap();
        assert_eq!(public_key1.as_hex(), KEY1_PUB_HEX);

        let priv_key2 = EddsaEd25519PrivateKey::from_hex(KEY2_PRIV_HEX).unwrap();
        assert_eq!(priv_key2.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(priv_key2.as_hex(), KEY2_PRIV_HEX);

        let public_key2 = context.get_public_key(&priv_key2).unwrap();
        assert_eq!(public_key2.as_hex(), KEY2_PUB_HEX);
    }

    #[test]
    fn check_invalid_digit() {
        let mut priv_chars: Vec<char> = KEY1_PRIV_HEX.chars().collect();
        priv_chars[3] = 'i';
        let priv_result =
            EddsaEd25519PrivateKey::from_hex(priv_chars.into_iter().collect::<String>().as_str());
        assert!(priv_result.is_err());

        let mut pub_chars: Vec<char> = KEY1_PUB_HEX.chars().collect();
        pub_chars[3] = 'i';
        let result =
            EddsaEd25519PublicKey::from_hex(pub_chars.into_iter().collect::<String>().as_str());
        assert!(result.is_err());
    }

    #[test]
    fn single_key_signing() {
        let context = create_context("eddsa_ed25519").unwrap();
        assert_eq!(context.get_algorithm_name(), "eddsa_ed25519");

        let factory = CryptoFactory::new(&*context);
        assert_eq!(factory.get_context().get_algorithm_name(), "eddsa_ed25519");

        let priv_key = EddsaEd25519PrivateKey::from_hex(KEY1_PRIV_HEX).unwrap();
        assert_eq!(priv_key.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(priv_key.as_hex(), KEY1_PRIV_HEX);

        let signer = factory.new_signer(&priv_key);
        let signature = signer.sign(&String::from(MSG1).into_bytes()).unwrap();
        assert_eq!(signature, MSG1_KEY1_SIG);
    }

    fn create_signer() -> Signer<'static> {
        let context = create_context("eddsa_ed25519").unwrap();
        assert_eq!(context.get_algorithm_name(), "eddsa_ed25519");

        let factory = CryptoFactory::new(&*context);
        assert_eq!(factory.get_context().get_algorithm_name(), "eddsa_ed25519");

        let priv_key = EddsaEd25519PrivateKey::from_hex(KEY1_PRIV_HEX).unwrap();
        assert_eq!(priv_key.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(priv_key.as_hex(), KEY1_PRIV_HEX);

        Signer::new_boxed(context, Box::new(priv_key))
    }

    #[test]
    fn single_key_signing_return_from_func() {
        let signer = create_signer();
        let signature = signer.sign(&String::from(MSG1).into_bytes()).unwrap();
        assert_eq!(signature, MSG1_KEY1_SIG);
    }

    #[test]
    fn many_key_signing() {
        let context = create_context("eddsa_ed25519").unwrap();
        assert_eq!(context.get_algorithm_name(), "eddsa_ed25519");

        let priv_key1 = EddsaEd25519PrivateKey::from_hex(KEY1_PRIV_HEX).unwrap();
        assert_eq!(priv_key1.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(priv_key1.as_hex(), KEY1_PRIV_HEX);

        let priv_key2 = EddsaEd25519PrivateKey::from_hex(KEY2_PRIV_HEX).unwrap();
        assert_eq!(priv_key2.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(priv_key2.as_hex(), KEY2_PRIV_HEX);

        let signature = context
            .sign(&String::from(MSG1).into_bytes(), &priv_key1)
            .unwrap();
        assert_eq!(signature, MSG1_KEY1_SIG);

        let signature = context
            .sign(&String::from(MSG2).into_bytes(), &priv_key2)
            .unwrap();
        assert_eq!(signature, MSG2_KEY2_SIG);
    }

    #[test]
    fn verification() {
        let context = create_context("eddsa_ed25519").unwrap();
        assert_eq!(context.get_algorithm_name(), "eddsa_ed25519");

        let pub_key1 = EddsaEd25519PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        assert_eq!(pub_key1.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(pub_key1.as_hex(), KEY1_PUB_HEX);

        let result = context.verify(
            MSG1_KEY1_SIG.as_ref(),
            &String::from(MSG1).into_bytes(),
            &pub_key1,
        );
        assert_eq!(result.unwrap(), true);
    }

    #[test]
    fn verification_error() {
        let context = create_context("eddsa_ed25519").unwrap();
        assert_eq!(context.get_algorithm_name(), "eddsa_ed25519");

        let pub_key1 = EddsaEd25519PublicKey::from_hex(KEY1_PUB_HEX).unwrap();
        assert_eq!(pub_key1.get_algorithm_name(), "eddsa_ed25519");
        assert_eq!(pub_key1.as_hex(), KEY1_PUB_HEX);

        // This signature doesn't match for MSG1/KEY1
        let result = context.verify(
            MSG2_KEY2_SIG.as_ref(),
            &String::from(MSG1).into_bytes(),
            &pub_key1,
        );
        assert_eq!(result.unwrap(), false);
    }
}
