use crate::encrypt::sm4::{decrypt, encrypt};
use crate::{bytes_to_hex_str, hex_str_to_bytes};
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use jni::JNIEnv;

#[no_mangle]
pub extern "C" fn Java_org_geno_kms_jni_Msp_encrypt(
    env: JNIEnv,
    class: JClass,
    input: JString,
) -> jstring {
    let input1: String = env
        .get_string(input)
        .expect("Couldn't get java string!")
        .into();
    let key = hex::decode("F29253377076E2A341FEE9F452D1C951").unwrap();
    let data = input1.as_bytes();
    let iv = hex::decode("fedcba0987654321fedcba0987654321").unwrap();

    let re = encrypt(data, &*key, &*iv).unwrap();
    let str = bytes_to_hex_str(&*re);

    let output = env.new_string(str).expect("Couldn't create java string!");
    output.into_inner()
}

#[no_mangle]
pub extern "C" fn Java_org_geno_kms_jni_Msp_decrypt(
    env: JNIEnv,
    class: JClass,
    input: JString,
) -> jstring {
    let input1: String = env
        .get_string(input)
        .expect("Couldn't get java string!")
        .into();
    let key = hex::decode("F29253377076E2A341FEE9F452D1C951").unwrap();
    let data = hex_str_to_bytes(&*input1).unwrap();
    let iv = hex::decode("fedcba0987654321fedcba0987654321").unwrap();

    let re = decrypt(&*data, &*key, &*iv).unwrap();
    let str = String::from_utf8(re).unwrap();

    let output = env.new_string(str).expect("Couldn't create java string!");
    output.into_inner()
}
