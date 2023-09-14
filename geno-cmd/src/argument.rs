
use clap::ValueEnum;


#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum ALG {
    Ed25519,
    Secp256k1,
    Sm2,
}

impl Into<&str> for ALG{
    fn into(self) -> &'static str {
        match self {
            ALG::Ed25519 => "eddsa_ed25519",
            ALG::Secp256k1 => "secp256k1",
            ALG::Sm2 => "sm2",
        }
    }
}