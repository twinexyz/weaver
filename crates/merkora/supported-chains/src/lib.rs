#[derive(Debug)]
pub enum Chains {
    Ethereum,
    Solana,
}

impl From<&str> for Chains {
    fn from(value: &str) -> Self {
        match value {
            "ethereum" => Chains::Ethereum,
            "solana" => Chains::Solana,
            _ => panic!("Invalid chain"),
        }
    }
}

impl From<String> for Chains {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_ref() {
            "ethereum" => Chains::Ethereum,
            "solana" => Chains::Solana,
            _ => panic!("Invalid chain"),
        }
    }
}
