use rand::{thread_rng, RngCore};
pub const SECRET_KEY_LENGTH: usize = 32;
pub fn rand() -> Vec<u8> {
    let mut csprng = thread_rng();
    let mut key = [0u8; SECRET_KEY_LENGTH];
    csprng.fill_bytes(&mut key);
    Vec::from(&key[..])
}
