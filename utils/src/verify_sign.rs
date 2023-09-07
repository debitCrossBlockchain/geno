use msp::hex_str_to_bytes;
use msp::signing::eddsa_ed25519::{EddsaEd25519Context, EddsaEd25519PublicKey};
use msp::signing::{create_private_key, Context, Error, PrivateKey, PublicKey};
use protos::common::Signature;

pub fn sign(private_key_str: &str, data: &[u8]) -> anyhow::Result<Signature> {
    let private_key = match create_private_key("eddsa_ed25519", private_key_str) {
        Ok(value) => value,
        Err(e) => return Err(anyhow::anyhow!("create_private_key error")),
    };

    let context = EddsaEd25519Context::new();
    match context.sign(data, private_key.as_ref()) {
        Ok(sign_ret) => {
            let mut sig: Signature = Signature::new();
            let pub_key = private_key.get_pubkey();
            let sign_data = match hex_str_to_bytes(sign_ret.as_str()) {
                Ok(value) => value,
                Err(e) => return Err(anyhow::anyhow!("hex_str_to_bytes error")),
            };
            let public_key = match hex_str_to_bytes(pub_key.as_str()) {
                Ok(value) => value,
                Err(e) => return Err(anyhow::anyhow!("hex_str_to_bytes error")),
            };
            sig.set_sign_data(Vec::from(sign_data));
            sig.set_public_key(public_key);
            Ok(sig)
        }
        Err(sign_err) => Err(anyhow::anyhow!("sign error")),
    }
}

pub fn verify_sign(sign: &protos::common::Signature, content: &[u8]) -> anyhow::Result<bool> {
    let ctx = EddsaEd25519Context::default();
    let pub_key = match EddsaEd25519PublicKey::from_bytes(sign.get_public_key()) {
        Ok(value) => value,
        Err(e) => return Err(anyhow::anyhow!("verify_sign create public key error")),
    };

    match ctx.verify(sign.get_sign_data(), content, &pub_key) {
        Ok(value) => Ok(value),
        Err(e) => Err(anyhow::anyhow!("verify_sign verify error")),
    }
}

pub fn get_sign_address(sign: &protos::common::Signature) -> anyhow::Result<String> {
    let pub_key = match EddsaEd25519PublicKey::from_bytes(sign.get_public_key()) {
        Ok(value) => value,
        Err(e) => return Err(anyhow::anyhow!("get_sign_address create public key error")),
    };
    Ok(pub_key.get_address())
}
