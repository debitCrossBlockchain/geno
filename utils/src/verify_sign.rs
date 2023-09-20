use msp::hex_str_to_bytes;
use msp::signing::eddsa_ed25519::{EddsaEd25519Context, EddsaEd25519PublicKey};
use msp::signing::{
    create_context, create_private_key, create_public_key_by_bytes, Context, Error, PrivateKey,
    PublicKey,
};
use protos::common::Signature;

pub fn sign(
    private_key_str: &str,
    data: &[u8],
    encryption_type: &str,
) -> anyhow::Result<Signature> {
    let private_key = match create_private_key(encryption_type, private_key_str) {
        Ok(value) => value,
        Err(e) => return Err(anyhow::anyhow!("create private key error")),
    };

    let context = match create_context(&*encryption_type) {
        Ok(v) => v,
        Err(e) => return Err(anyhow::anyhow!("{:?}", e)),
    };
    match context.sign(data, private_key.as_ref()) {
        Ok(sign_ret) => {
            let mut signature: Signature = Signature::new();
            let pub_key = private_key.get_pubkey();
            let sign_data = match hex_str_to_bytes(sign_ret.as_str()) {
                Ok(value) => value,
                Err(e) => return Err(anyhow::anyhow!("sign data decode error")),
            };
            let public_key = match hex_str_to_bytes(pub_key.as_str()) {
                Ok(value) => value,
                Err(e) => return Err(anyhow::anyhow!("publish key decode  error")),
            };
            signature.set_sign_data(Vec::from(sign_data));
            signature.set_public_key(public_key);
            signature.set_encryption_type(encryption_type.to_string());
            Ok(signature)
        }
        Err(_) => Err(anyhow::anyhow!("signature error")),
    }
}

pub fn verify_sign(signature: &Signature, content: &[u8]) -> anyhow::Result<bool> {
    let ctx = match create_context(signature.get_encryption_type()) {
        Ok(v) => v,
        Err(e) => return Err(anyhow::anyhow!("{:?}", e)),
    };

    let pub_key = match create_public_key_by_bytes(
        signature.get_encryption_type(),
        signature.get_public_key(),
    ) {
        Ok(v) => v,
        Err(e) => return Err(anyhow::anyhow!("{:?}", e)),
    };

    match ctx.verify(signature.get_sign_data(), content, &*pub_key) {
        Ok(value) => Ok(value),
        Err(e) => Err(anyhow::anyhow!("verify signature error")),
    }
}

pub fn get_sign_address(signature: &Signature) -> anyhow::Result<String> {
    let pub_key = match create_public_key_by_bytes(
        signature.get_encryption_type(),
        signature.get_public_key(),
    ) {
        Ok(v) => v,
        Err(e) => return Err(anyhow::anyhow!("{:?}", e)),
    };
    Ok(pub_key.get_address())
}
