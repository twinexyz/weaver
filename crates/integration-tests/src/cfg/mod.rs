use std::fs;
use std::path::Path;

use serde::Deserialize;

/// Represents the full configuration file.
#[derive(Debug, Deserialize, Clone)]
pub struct TestConfig {
    pub nodes: NodesConfig,
    pub merkora: MerkoraConfig,
    pub smart_contracts: SmartContractsConfig,
    pub test_scripts: TestScripts,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NodesConfig {
    pub reth: NodeConfig,
    pub solana: NodeConfig,
    pub l2: NodeConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NodeConfig {
    #[serde(rename = "type_")]
    pub node_type: String, // "evm" | "solana"
    pub name: String,
    pub binary_name: String,
    pub genesis_path: Option<String>, // Only twine node needs this
}

/// If you give repository path, you can skip:
/// - url: [github url]
/// - branch: [github branch]
/// - build command: [command to build merkora]
#[derive(Debug, Deserialize, Clone)]
pub struct MerkoraConfig {
    pub name: String,
    pub url: Option<String>,
    pub branch: Option<String>,
    pub build: Option<bool>,
    pub repo_path: Option<String>,
    pub migrations_path: Option<String>,
    pub database_url: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SmartContractsConfig {
    pub solidity: ContractRepoConfig,
    pub solana: ContractRepoConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ContractRepoConfig {
    pub repo_path: Option<String>,
    pub url: Option<String>,
    pub name: Option<String>,
    pub branch: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct TestScripts {
    pub path: String,
}

/// Load a configuration from a YAML file.
pub fn load_config<P: AsRef<Path>>(path: P) -> eyre::Result<TestConfig> {
    let content = fs::read_to_string(path)?;
    let config: TestConfig = serde_yaml::from_str(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_config() {
        let yaml = r#"
nodes:
  reth:
    type_: "evm"
    name: "reth-node"
    binary_name: "reth"
  solana:
    type_: "solana"
    name: "solana-test-validator"
    binary_name: "solana-test-validator"
  l2:
    type_: "evm"
    name: "l2-node"
    binary_name: "twine-node"
    genesis_path: "/tmp/dev-genesis.json"

merkora:
  name: "merkora"
  url: "git@github.com:twinexyz/merkora.git"
  branch: "main"
  build_command: "cargo build --release"
  binary_path: "merkora"
  migrations_path: "/path/to/merkora/config.yaml"
  database_url: "postgresql://user:password@localhost:5432/relayer_db"

smart_contracts:
  solidity:
    url: "git@github.com:twinexyz/twine-solidity-contracts.git"
    name: "twine-solidity-contracts"
    branch: "main"

test_scripts:
  path: "./scripts"
"#;

        let cfg: TestConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(cfg.nodes.reth.name, "reth-node");
        assert_eq!(cfg.smart_contracts.solidity.branch.unwrap(), "main");
    }
}
