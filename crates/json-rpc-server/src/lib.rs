use async_trait::async_trait;
use jsonrpsee::proc_macros::rpc;
use jsonrpsee::server::Server;
use jsonrpsee::types::{ErrorCode, ErrorObject, ErrorObjectOwned};
use twine_types::manager::ChainManager;
use twine_types::ChainType;

pub struct JsonRpcServer {
    manager: ChainManager,
}

#[rpc(server, client)]
pub trait Rpc {
    #[method(name = "twmer_sendProof")]
    async fn batch_info(&self, param: ChainType) -> Result<(), ErrorObjectOwned>;
}

#[async_trait]
impl RpcServer for JsonRpcServer {
    async fn batch_info(&self, param: ChainType) -> Result<(), ErrorObjectOwned> {
        tracing::info!("Received proof");
        let manager = &self.manager;
        let identifier = param.get_chain_identifier();

        if let Some(sender) = manager.get_sender(&identifier) {
            match sender.send(param).await {
                Ok(_) => return Ok(()),
                Err(e) =>
                    return Err(ErrorObject::owned(
                        ErrorCode::InternalError.code(),
                        format!("internal error: {:?}", e),
                        Some(ErrorCode::InternalError),
                    )),
            }
        } else {
            return Err(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "internal error: check `chain_id` is correct".to_string(),
                Some(ErrorCode::InternalError),
            ));
        }
    }
}

impl JsonRpcServer {
    pub fn new(manager: ChainManager) -> JsonRpcServer { JsonRpcServer { manager } }

    pub async fn run(self, port: u64) -> eyre::Result<()> {
        let addr = format!("0.0.0.0:{}", port);
        tracing::info!("JSON RPC server running at {}", addr);
        let server = Server::builder().build(addr).await?;
        let handle = server.start(self.into_rpc());
        handle.stopped().await;
        Ok(())
    }
}
