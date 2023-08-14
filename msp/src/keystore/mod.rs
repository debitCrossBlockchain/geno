use crate::encrypt;
use crate::signing;
use crate::signing::PrivateKey;
use crate::signing::{bytes_to_hex_str, hex_str_to_bytes};
use crate::utils::get_strong_rand_bytes;
use crypto::scrypt::{scrypt, ScryptParams};
use rand::Rng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::str;

/**
"scrypt_params" : {"log_n": 14, "p": 8, "r" : 1},//n:16384
}
**/

#[derive(Serialize, Deserialize, Debug)]
pub struct KeyStore {
    address: String,
    sign_type: String,
    params_log_n: u8,
    params_r: u32,
    params_p: u32,
    salt: String,
    aes_iv: String,
    aes_key: String,
    cypher_text: String,
}
impl KeyStore {
    pub fn new() -> Self {
        Self {
            address: "".to_string(),
            sign_type: "".to_string(),
            params_log_n: 14,
            params_r: 1,
            params_p: 8,
            salt: "".to_string(),
            aes_iv: "".to_string(),
            aes_key: "".to_string(),
            cypher_text: "".to_string(),
        }
    }
    pub fn generate(
        password: &str,
        key_store: &mut KeyStore,
        new_priv_key: &Box<dyn PrivateKey>,
        algorithm_name: &str,
    ) -> bool {
        let mut rng = rand::thread_rng();
        let n1: u32 = rng.gen();
        let n2: u32 = rng.gen();
        // let password = get_strong_rand_bytes(&*n1.to_string());
        let salt = get_strong_rand_bytes(&*n2.to_string());
        let params_log_n = key_store.params_log_n;
        let params_r = key_store.params_r;
        let params_p = key_store.params_p;
        let mut dk: [u8; 32] = [0; 32];
        // let mut dk: [u8; 16] = [0; 16];
        let scrypt_params = ScryptParams::new(params_log_n, params_r, params_p);
        scrypt(
            (&password).as_ref(),
            (&salt).as_ref(),
            &scrypt_params,
            &mut dk,
        );

        // let mut key: [u8; 16] = [0; 16];
        let mut key: [u8; 32] = [0; 32];
        let mut iv: [u8; 16] = [0; 16];
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut iv);
        let priv_key = new_priv_key;
        let private_key_string = priv_key.as_hex();
        let private_key = priv_key.as_slice();
        let public_address = priv_key.get_address();
        let public_key = priv_key.get_pubkey();
        let cypher_text = encrypt::encrypt_aes(private_key, &key, &iv).ok().unwrap();
        // let cypher_text = encrypt::encrypt_sm4(private_key, &key, &iv).ok().unwrap();
        let sign_type = priv_key.get_algorithm_name().to_string();
        key_store.salt = salt;
        key_store.aes_iv = bytes_to_hex_str(&iv);
        key_store.aes_key = bytes_to_hex_str(&key);
        // key_store.aes_iv = iv;
        // key_store.aes_key = key;
        key_store.cypher_text = bytes_to_hex_str(cypher_text.as_slice());
        key_store.address = public_address;
        key_store.sign_type = sign_type;
        // key_store.private_key = private_key_string;
        // key_store.public_key = public_key;
        return true;
    }
    pub fn from(key_store: KeyStore, password: &str, new_priv_key: &mut String) -> bool {
        let params_log_n = key_store.params_log_n;
        let params_r = key_store.params_r;
        let params_p = key_store.params_p;
        let algorithm_name = key_store.sign_type;
        let salt = key_store.salt;
        let aes_iv = key_store.aes_iv;
        let aes_key = key_store.aes_key;
        let mut dk: [u8; 32] = [0; 32];
        // let mut dk: [u8; 16] = [0; 16];
        let address = key_store.address;
        let cipher = key_store.cypher_text;
        let scrypt_params = ScryptParams::new(params_log_n, params_r, params_p);
        let aes_key_vec = hex_str_to_bytes(&aes_key).unwrap();
        let aes_key_u8 = aes_key_vec.as_slice();
        let aes_iv_vec = hex_str_to_bytes(&aes_iv).unwrap();
        let aes_iv_u8 = aes_iv_vec.as_slice();
        let cypher_text_vec = hex_str_to_bytes(&*cipher).unwrap();
        let cyper_text_u8 = cypher_text_vec.as_slice();

        let ret = scrypt(
            (&password).as_ref(),
            (&salt).as_ref(),
            &scrypt_params,
            &mut dk,
        );
        if ret != () {
            return false;
        }
        let priv_key_de = encrypt::decrypt_aes(cyper_text_u8, aes_key_u8, aes_iv_u8)
        // let priv_key_de = encrypt::decrypt_sm4(cyper_text_u8, aes_key_u8, aes_iv_u8)
            .ok()
            .unwrap();
        let priv_key_de_u8 = &priv_key_de.as_slice();
        let mut priv_key_de_string = bytes_to_hex_str(priv_key_de_u8);
        let priv_key = signing::create_private_key(&*algorithm_name, &priv_key_de_string).unwrap();
        let public_address = priv_key.get_address();
        if public_address != address {
            return false;
        }
        new_priv_key.push_str(&priv_key_de_string);
        true
    }
}
