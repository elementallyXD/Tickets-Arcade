use std::env;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub start_block: u64,
    pub database_url: String,
    pub raffle_factory_address: String,
    pub randomness_provider_address: Option<String>,
    pub explorer_base_url: String,
    pub bind_addr: String,
    pub indexer_batch_size: u64,
    pub indexer_poll_interval_ms: u64,
}

impl AppConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let rpc_url = env::var("RPC_URL")
            .unwrap_or_else(|_| "https://rpc.testnet.arc.network".to_string());
        let chain_id = env::var("CHAIN_ID")
            .unwrap_or_else(|_| "5042002".to_string())
            .parse()?;
        let start_block = env::var("START_BLOCK")
            .unwrap_or_else(|_| "0".to_string())
            .parse()?;
        let database_url = env::var("DATABASE_URL")?;
        let raffle_factory_address = env::var("RAFFLE_FACTORY_ADDRESS")?;
        let randomness_provider_address = env::var("RANDOMNESS_PROVIDER_ADDRESS").ok();
        let explorer_base_url = env::var("EXPLORER_BASE_URL")
            .unwrap_or_else(|_| "https://testnet.arcscan.app".to_string());
        let bind_addr =
            env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
        let indexer_batch_size = env::var("INDEXER_BATCH_SIZE")
            .unwrap_or_else(|_| "2000".to_string())
            .parse()?;
        let indexer_poll_interval_ms = env::var("INDEXER_POLL_INTERVAL_MS")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()?;

        Ok(Self {
            rpc_url,
            chain_id,
            start_block,
            database_url,
            raffle_factory_address,
            randomness_provider_address,
            explorer_base_url,
            bind_addr,
            indexer_batch_size,
            indexer_poll_interval_ms,
        })
    }
}
