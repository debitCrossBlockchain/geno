pub mod eddsa_ed25519;
pub mod secp256k1;
pub mod sm2;
use crate::signing::eddsa_ed25519::{
    EddsaEd25519Context, EddsaEd25519PrivateKey, EddsaEd25519PublicKey,
};
use crate::signing::secp256k1::{Secp256k1Context, Secp256k1PrivateKey, Secp256k1PublicKey};
use crate::signing::sm2::{Sm2Context, Sm2PrivateKey, Sm2PublicKey};
use libsm::sm2::signature::Signature;
use std::error::Error as StdError;
use tiny_keccak::keccak256;

pub const ADDRESS_PREFIX: &str = "did:gdt:0x";
pub const ADDRESS_LENGTH: usize = 40;
pub const PRIVATE_KEY_LENGTH: usize = 64;
pub const ED25519_PUBLIC_KEY_LENGTH: usize = 64;
pub const SM2_PUBLIC_KEY_LENGTH: usize = 66;
pub const SECP256K1_PUBLIC_KEY_LENGTH: usize = 66;
pub const TX_HASH_LENGTH: usize = 64;
// pub const  TX_HASH_PREFIX : usize = 2;
pub const SIGNATURE_LENGTH_MIN: usize = 128;
pub const SIGNATURE_LENGTH_MAX: usize = 256;

#[derive(Debug)]
pub enum Error {
    /// Returned when trying to create an algorithm which does not exist.
    NoSuchAlgorithm(String),
    /// Returned when an error occurs during deserialization of a Private or
    /// Public key from various formats.
    ParseError(String),
    /// Returned when an error occurs during the signing process.
    SigningError(Box<dyn StdError>),
    /// Returned when an error occurs during key generation
    KeyGenError(String),

    AddressError(String),
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::SigningError(err) => Some(&**err),
            _ => None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Error::NoSuchAlgorithm(ref s) => write!(f, "NoSuchAlgorithm: {}", s),
            Error::ParseError(ref s) => write!(f, "ParseError: {}", s),
            Error::SigningError(ref err) => write!(f, "SigningError: {}", err),
            Error::KeyGenError(ref s) => write!(f, "KeyGenError: {}", s),
            Error::AddressError(ref s) => write!(f, "AddressError: {}", s),
        }
    }
}

/// A private key instance.
/// The underlying content is dependent on implementation.
pub trait PrivateKey {
    /// Returns the algorithm name used for this private key.
    fn get_algorithm_name(&self) -> &str;
    /// Return the private key encoded as a hex string.
    fn as_hex(&self) -> String;
    /// Return the private key bytes.
    fn as_slice(&self) -> &[u8];
    /// Return the address String.
    fn get_address(&self) -> String {
        // let context = create_context(self
        //     .get_algorithm_name()).unwrap();
        // String::from(bs58::encode(
        //     context
        //         .get_public_key(&*create_private_key(self.get_algorithm_name(), self.as_hex().as_str()).unwrap()
        //         ).unwrap().as_slice())
        //     .into_string())

        let context = create_context(self.get_algorithm_name()).unwrap();
        let public_key = context
            .get_public_key(
                &*create_private_key(self.get_algorithm_name(), self.as_hex().as_str()).unwrap(),
            )
            .unwrap();
        public_key.get_address()
        // let hash = keccak256(&context.get_public_key(&*create_private_key(self.get_algorithm_name(), self.as_hex().as_str()).unwrap()
        // ).unwrap().as_slice()[1..]);
        // let address = bytes_to_hex_str(&hash[12..]).to_lowercase();
        // let hash = bytes_to_hex_str(address.as_bytes());
        // let mut checksum_address = ADDRESS_PREFIX.to_string();
        // for c in 0..40 {
        //     let ch = match &hash[c..=c] {
        //         "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" => address[c..=c].to_lowercase(),
        //         _ => address[c..=c].to_uppercase(),
        //     };
        //     checksum_address.push_str(&ch);
        // }
        //
        // checksum_address
        // let mut s1 = "did:geno:".to_string();
        // let s2 = String::from(bs58::encode(
        //     context
        //         .get_public_key(&*create_private_key(self.get_algorithm_name(), self.as_hex().as_str()).unwrap()
        //         ).unwrap().as_slice())
        //     .into_string());
        // s1 += &s2;
        // s1
    }
    /// Return the address String.
    fn get_pubkey(&self) -> String {
        let context = create_context(self.get_algorithm_name()).unwrap();
        context
            .get_public_key(
                &*create_private_key(self.get_algorithm_name(), self.as_hex().as_str()).unwrap(),
            )
            .unwrap()
            .as_hex()
    }

    fn sign_data(&self, data: &[u8], algorithm_name: &str) -> String {
        if algorithm_name == "secp256k1" {
            let ctx = Secp256k1Context::new();
            let signature = ctx
                .sign(
                    data,
                    &*create_private_key(self.get_algorithm_name(), self.as_hex().as_str())
                        .unwrap(),
                )
                .unwrap();
            signature
        } else if algorithm_name == "eddsa_ed25519" {
            let ctx = EddsaEd25519Context::new();
            let signature = ctx
                .sign(
                    data,
                    &*create_private_key(self.get_algorithm_name(), self.as_hex().as_str())
                        .unwrap(),
                )
                .unwrap();
            signature
        } else if algorithm_name == "sm2" {
            let ctx = Sm2Context::new();
            let signature = ctx
                .sign(
                    data,
                    &*create_private_key(self.get_algorithm_name(), self.as_hex().as_str())
                        .unwrap(),
                )
                .unwrap();
            signature
        } else {
            panic!("Invalid algorithm name: {}", algorithm_name);
        }
    }
}

/// A public key instance.
/// The underlying content is dependent on implementation.
pub trait PublicKey {
    /// Returns the algorithm name used for this public key.
    fn get_algorithm_name(&self) -> &str;
    /// Return the public key encoded as a hex string.
    fn as_hex(&self) -> String;
    /// Return the public key bytes.
    fn as_slice(&self) -> &[u8];
    /// Return the address String.
    fn get_address(&self) -> String {
        let hash = keccak256(&self.as_slice()[1..]);
        let address = bytes_to_hex_str(&hash[12..]).to_lowercase();
        let mut checksum_address = ADDRESS_PREFIX.to_string();
        checksum_address.push_str(&address);
        checksum_address
        // let hash = bytes_to_hex_str(address.as_bytes());
        // let mut checksum_address = ADDRESS_PREFIX.to_string();
        // for c in 0..40 {
        //     let ch = match &hash[c..=c] {
        //         "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" => address[c..=c].to_lowercase(),
        //         _ => address[c..=c].to_lowercase(),
        //     };
        //     checksum_address.push_str(&ch);
        // }
        //
        // checksum_address
    }
}

/// A context for a cryptographic signing algorithm.
pub trait Context {
    /// Returns the algorithm name.
    fn get_algorithm_name(&self) -> &str;
    /// Sign a message
    /// Given a private key for this algorithm, sign the given message bytes
    /// and return a hex-encoded string of the resulting signature.
    /// # Arguments
    ///
    /// * `message`- the message bytes
    /// * `private_key` the private key
    ///
    /// # Returns
    ///
    /// * `signature` - The signature in a hex-encoded string
    fn sign(&self, message: &[u8], key: &dyn PrivateKey) -> Result<String, Error>;

    /// Verifies that the signature of a message was produced with the
    /// associated public key.
    /// # Arguments
    ///
    /// * `signature` - the hex-encoded signature
    /// * `message` - the message bytes
    /// * `public_key` - the public key to use for verification
    ///
    /// # Returns
    ///
    /// * `boolean` - True if the public key is associated with the signature for that method,
    ///            False otherwise
    fn verify(&self, signature: &[u8], message: &[u8], key: &dyn PublicKey) -> Result<bool, Error>;

    /// Produce the public key for the given private key.
    /// # Arguments
    ///
    /// `private_key` - a private key
    ///
    /// # Returns
    /// * `public_key` - the public key for the given private key
    fn get_public_key(&self, private_key: &dyn PrivateKey) -> Result<Box<dyn PublicKey>, Error>;

    ///Generates a new random PrivateKey using this context.
    /// # Returns
    ///
    /// * `private_key` - a random private key
    fn new_random_private_key(&self) -> Result<Box<dyn PrivateKey>, Error>;
}

pub fn create_secret_key(algorithm_name: &str) -> Result<Box<dyn PrivateKey>, Error> {
    match algorithm_name {
        "secp256k1" => Ok(Box::new(Secp256k1PrivateKey::new())),
        "eddsa_ed25519" => Ok(Box::new(EddsaEd25519PrivateKey::new())),
        "sm2" => Ok(Box::new(Sm2PrivateKey::new())),
        _ => Err(Error::NoSuchAlgorithm(format!(
            "no such algorithm: {}",
            algorithm_name
        ))),
    }
}

pub fn create_private_key(algorithm_name: &str, key: &str) -> Result<Box<dyn PrivateKey>, Error> {
    match algorithm_name {
        "secp256k1" => Ok(Box::new(Secp256k1PrivateKey::from_hex(key).unwrap())),
        "eddsa_ed25519" => Ok(Box::new(EddsaEd25519PrivateKey::from_hex(key).unwrap())),
        "sm2" => Ok(Box::new(Sm2PrivateKey::from_hex(key).unwrap())),
        _ => Err(Error::NoSuchAlgorithm(format!(
            "no such algorithm: {}",
            algorithm_name
        ))),
    }
}

pub fn create_public_key(algorithm_name: &str, key: &str) -> Result<Box<dyn PublicKey>, Error> {
    match algorithm_name {
        "secp256k1" => {
            let result = Secp256k1PublicKey::from_hex(key);
            if result.is_err() {
                return Err(Error::ParseError(format!("key err")));
            }
            Ok(Box::new(result.unwrap()))
        }
        "eddsa_ed25519" => {
            let result = EddsaEd25519PublicKey::from_hex(key);
            if result.is_err() {
                return Err(Error::ParseError(format!("key err")));
            }
            Ok(Box::new(result.unwrap()))
        }
        "sm2" => {
            let result = Sm2PublicKey::from_hex(key);
            if result.is_err() {
                return Err(Error::ParseError(format!("key err")));
            }
            Ok(Box::new(result.unwrap()))
        }
        _ => Err(Error::NoSuchAlgorithm(format!(
            "no such algorithm: {}",
            algorithm_name
        ))),
    }
}

pub fn create_public_key_by_bytes(
    algorithm_name: &str,
    key: &[u8],
) -> Result<Box<dyn PublicKey>, Error> {
    match algorithm_name {
        "secp256k1" => {
            let result = Secp256k1PublicKey::from_bytes(key);
            if result.is_err() {
                return Err(Error::ParseError(format!("key err")));
            }
            Ok(Box::new(result.unwrap()))
        }
        "eddsa_ed25519" => {
            let result = EddsaEd25519PublicKey::from_bytes(key);
            if result.is_err() {
                return Err(Error::ParseError(format!("key err")));
            }
            Ok(Box::new(result.unwrap()))
        }
        "sm2" => {
            let result = Sm2PublicKey::from_bytes(key);
            if result.is_err() {
                return Err(Error::ParseError(format!("key err")));
            }
            Ok(Box::new(result.unwrap()))
        }
        _ => Err(Error::NoSuchAlgorithm(format!(
            "no such algorithm: {}",
            algorithm_name
        ))),
    }
}

pub fn create_context(algorithm_name: &str) -> Result<Box<dyn Context>, Error> {
    match algorithm_name {
        "secp256k1" => Ok(Box::new(secp256k1::Secp256k1Context::new())),
        "eddsa_ed25519" => Ok(Box::new(eddsa_ed25519::EddsaEd25519Context::new())),
        "sm2" => Ok(Box::new(sm2::Sm2Context::new())),
        _ => Err(Error::NoSuchAlgorithm(format!(
            "no such algorithm: {}",
            algorithm_name
        ))),
    }
}

/// Factory for generating signers.
pub struct CryptoFactory<'a> {
    context: &'a dyn Context,
}

impl<'a> CryptoFactory<'a> {
    /// Constructs a CryptoFactory.
    /// # Arguments
    ///
    /// * `context` - a cryptographic context
    pub fn new(context: &'a dyn Context) -> Self {
        CryptoFactory { context }
    }

    /// Returns the context associated with this factory
    ///
    /// # Returns
    ///
    /// * `context` - a cryptographic context
    pub fn get_context(&self) -> &dyn Context {
        self.context
    }

    /// Create a new signer for the given private key.
    ///
    /// # Arguments
    ///
    /// `private_key` - a private key
    ///
    /// # Returns
    ///
    /// * `signer` - a signer instance
    pub fn new_signer(&self, key: &'a dyn PrivateKey) -> Signer {
        Signer::new(self.context, key)
    }
}

enum ContextAndKey<'a> {
    ByRef(&'a dyn Context, &'a dyn PrivateKey),
    ByBox(Box<dyn Context>, Box<dyn PrivateKey>),
}

/// A convenient wrapper of Context and PrivateKey
pub struct Signer<'a> {
    context_and_key: ContextAndKey<'a>,
}

impl<'a> Signer<'a> {
    /// Constructs a new Signer
    ///
    /// # Arguments
    ///
    /// * `context` - a cryptographic context
    /// * `private_key` - private key
    pub fn new(context: &'a dyn Context, key: &'a dyn PrivateKey) -> Self {
        Signer {
            context_and_key: ContextAndKey::ByRef(context, key),
        }
    }

    /// Constructs a new Signer with boxed arguments
    ///
    /// # Arguments
    ///
    /// * `context` - a cryptographic context
    /// * `key` - private key
    pub fn new_boxed(context: Box<dyn Context>, key: Box<dyn PrivateKey>) -> Self {
        Signer {
            context_and_key: ContextAndKey::ByBox(context, key),
        }
    }

    /// Signs the given message.
    ///
    /// # Arguments
    ///
    /// * `message` - the message bytes
    ///
    /// # Returns
    ///
    /// * `signature` - the signature in a hex-encoded string
    pub fn sign(&self, message: &[u8]) -> Result<String, Error> {
        match &self.context_and_key {
            ContextAndKey::ByRef(context, key) => context.sign(message, *key),
            ContextAndKey::ByBox(context, key) => context.sign(message, key.as_ref()),
        }
    }

    /// Return the public key for this Signer instance.
    ///
    /// # Returns
    ///
    /// * `public_key` - the public key instance
    pub fn get_public_key(&self) -> Result<Box<dyn PublicKey>, Error> {
        match &self.context_and_key {
            ContextAndKey::ByRef(context, key) => context.get_public_key(*key),
            ContextAndKey::ByBox(context, key) => context.get_public_key(key.as_ref()),
        }
    }
}

pub fn hex_str_to_bytes(s: &str) -> Result<Vec<u8>, Error> {
    for (i, ch) in s.chars().enumerate() {
        if !ch.is_digit(16) {
            return Err(Error::ParseError(format!(
                "invalid character position {}",
                i
            )));
        }
    }

    let input: Vec<_> = s.chars().collect();

    let decoded: Vec<u8> = input
        .chunks(2)
        .map(|chunk| {
            ((chunk[0].to_digit(16).unwrap() << 4) | (chunk[1].to_digit(16).unwrap())) as u8
        })
        .collect();

    Ok(decoded)
}

pub fn bytes_to_hex_str(b: &[u8]) -> String {
    b.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join("")
}

pub fn verify_data(
    signed_data: &str,
    message: &[u8],
    public_key: &str,
    algorithm_name: &str,
) -> bool {
    if algorithm_name == "secp256k1" {
        let ctx = Secp256k1Context::default();
        let pub_key = Secp256k1PublicKey::from_hex(public_key).unwrap();
        let signature = hex_str_to_bytes(signed_data).unwrap();
        let signature_u8 = signature.as_slice();
        let ret = ctx.verify(signature_u8, message, &pub_key).unwrap();
        ret
    } else if algorithm_name == "eddsa_ed25519" {
        let ctx = EddsaEd25519Context::default();
        let pub_key = EddsaEd25519PublicKey::from_hex(public_key).unwrap();
        let signature = hex_str_to_bytes(signed_data).unwrap();
        let signature_u8 = signature.as_slice();
        let ret = ctx.verify(signature_u8, message, &pub_key).unwrap();
        ret
    } else if algorithm_name == "sm2" {
        let ctx = Sm2Context::default();
        let pub_key = Sm2PublicKey::from_hex(public_key).unwrap();
        let signature = hex_str_to_bytes(signed_data).unwrap();
        let signature_u8 = signature.as_slice();
        let ret = ctx.verify(signature_u8, message, &pub_key).unwrap();
        ret
    } else {
        panic!("Invalid algorithm name: {}", algorithm_name);
    }
}
pub fn check_private_key(mut private_key: &str) -> bool {
    if private_key.len() != PRIVATE_KEY_LENGTH {
        return false;
    }
    return is_hex(private_key);
}

pub fn check_public_key(algorithm_name: &str, mut public_key: &str) -> bool {
    match algorithm_name {
        "secp256k1" => {
            if public_key.len() != SECP256K1_PUBLIC_KEY_LENGTH {
                return false;
            }
        }
        "eddsa_ed25519" => {
            if public_key.len() != ED25519_PUBLIC_KEY_LENGTH {
                return false;
            }
        }
        "sm2" => {
            if public_key.len() != SM2_PUBLIC_KEY_LENGTH {
                return false;
            }
        }
        _ => return false,
    }

    return is_hex(public_key);
}
pub fn check_hex(mut hex: &str) -> bool {
    return hex.len() % 2 == 0 && is_hex(hex);
}
pub fn check_address(address: &str) -> bool {
    let mut new_address = address;
    if address.len() == (ADDRESS_PREFIX.len() + ADDRESS_LENGTH)
        && &address[0..ADDRESS_PREFIX.len()] == ADDRESS_PREFIX
    {
        new_address = &address[ADDRESS_PREFIX.len()..];
    } else {
        return false;
    }
    return new_address.len() == ADDRESS_LENGTH && is_hex(new_address);
}
pub fn check_signature(algorithm_name: &str, signature: &str) -> bool {
    if signature.len() % 2 != 0 {
        return false;
    }
    match algorithm_name {
        "secp256k1" => {
            if signature.len() < SIGNATURE_LENGTH_MIN || signature.len() > SIGNATURE_LENGTH_MAX {
                return false;
            }
        }
        "eddsa_ed25519" => {
            if signature.len() < SIGNATURE_LENGTH_MIN || signature.len() > SIGNATURE_LENGTH_MAX {
                return false;
            }
        }
        "sm2" => {
            if signature.len() < SIGNATURE_LENGTH_MIN || signature.len() > SIGNATURE_LENGTH_MAX {
                return false;
            }
            // let sig = Signature::der_decode(signature.as_ref());
            // if sig.is_err() {
            //     return false;
            // }
        }
        _ => return false,
    }

    return is_hex(signature);
}

pub fn check_tx_hash(hash: &str) -> bool {
    return hash.len() == TX_HASH_LENGTH && is_hex(hash);
}

pub fn is_hex(address: &str) -> bool {
    for i in address.chars() {
        if !(('0' <= i && i <= '9') || ('a' <= i && i <= 'f')) {
            return false;
        }
    }
    true
}
#[cfg(test)]
mod signing_test {
    use super::create_context;
    use crate::signing::check_address;

    #[test]
    fn no_such_algorithm() {
        let result = create_context("invalid");
        assert!(result.is_err())
    }
    #[test]
    fn test_check_address() {
        let address = "did:gdt:0x6d99abe0ee7a7a5cf137ce729ad4545fa3c5dfad";
        let result = check_address(address);

        println!("{}", result);
    }
}
