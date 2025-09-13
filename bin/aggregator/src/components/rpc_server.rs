//! JSON RPC server for receiving proofs as a backup to Kafka

use std::net::SocketAddr;

use eyre::Result;
use jsonrpsee::core::{async_trait, RpcResult};
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::{ServerBuilder, ServerHandle};
use reth_tracing::tracing::{error, info};
use sqlx::PgPool;
use twine_aggregator_consumer::{process_proof, ProofSource};
use twine_types::proofs::ZkProof;

/// RPC interface for submitting proofs
#[rpc(server, namespace = "twagg")]
trait ProofApi {
    /// Submit a proof for a batch
    #[method(name = "submitProof")]
    async fn submit_proof(&self, proof: ZkProof) -> RpcResult<()>;

    /// Health check endpoint
    #[method(name = "health")]
    fn health(&self) -> RpcResult<String>;
}

/// Implementation of the proof API
pub(crate) struct ProofApiImpl {
    db_pool: PgPool,
    twine_rpc_url: String,
    twine_chain_id: u64,
}

impl ProofApiImpl {
    pub(crate) fn new(db_pool: PgPool, twine_rpc_url: String, twine_chain_id: u64) -> Self {
        Self {
            db_pool,
            twine_rpc_url,
            twine_chain_id,
        }
    }
}

#[async_trait]
impl ProofApiServer for ProofApiImpl {
    async fn submit_proof(&self, proof: ZkProof) -> RpcResult<()> {
        info!("Received proof submission via RPC");

        // Process the proof using the shared consumer logic
        match process_proof(
            &self.db_pool,
            &self.twine_rpc_url,
            proof,
            ProofSource::JsonRpc,
            self.twine_chain_id,
        )
        .await
        {
            Ok(_) => {
                info!("Successfully processed proof via RPC");
                Ok(())
            }
            Err(e) => {
                error!("Failed to process proof via RPC: {:?}", e);
                Err(jsonrpsee::types::ErrorObject::owned(
                    jsonrpsee::types::ErrorCode::InternalError.code(),
                    format!("Failed to process proof: {}", e),
                    None::<()>,
                ))
            }
        }
    }

    fn health(&self) -> RpcResult<String> { Ok("ok".to_string()) }
}

/// Start the JSON RPC server and return a handle for graceful shutdown
pub(crate) async fn start_rpc_server(
    config: &twine_aggregator_common::config::AppCfg,
    db_pool: PgPool,
) -> Result<ServerHandle> {
    info!("Starting JSON RPC server");

    let rpc_config = &config.rpc;
    let addr: SocketAddr = format!("{}:{}", rpc_config.host, rpc_config.port)
        .parse()
        .map_err(|e| eyre::eyre!("Invalid RPC server address: {}", e))?;

    let rpc_impl = ProofApiImpl::new(db_pool, config.twine.rpc.clone(), config.twine.chain_id);

    let server = ServerBuilder::default()
        .build(addr)
        .await
        .map_err(|e| eyre::eyre!("Failed to build RPC server: {}", e))?;

    let module = ProofApiServer::into_rpc(rpc_impl);

    info!("RPC server configured on {}", addr);

    let handle = server.start(module);

    info!("JSON RPC server started");

    Ok(handle)
}
