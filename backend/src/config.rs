//! Application configuration loaded from environment variables
//!
//! # Security Considerations
//! - DATABASE_URL contains credentials and is never logged
//! - All required variables are validated on startup
//! - Default values are safe fallbacks for development

use std::env;

/// Application configuration loaded from environment variables
///
/// Required environment variables:
/// - `DATABASE_URL` - PostgreSQL connection string (contains credentials)
/// - `RAFFLE_FACTORY_ADDRESS` - Deployed RaffleFactory contract address
///
/// Optional environment variables with defaults:
/// - `RPC_URL` - Arc testnet RPC URL (default: https://rpc.testnet.arc.network)
/// - `CHAIN_ID` - Expected chain ID (default: 5042002)
/// - `START_BLOCK` - Block to start indexing from (default: 0)
/// - `EXPLORER_BASE_URL` - Block explorer URL (default: https://testnet.arcscan.app)
/// - `BIND_ADDR` - Server bind address (default: 0.0.0.0:8080)
/// - `INDEXER_BATCH_SIZE` - Blocks per indexing batch (default: 2000)
/// - `INDEXER_POLL_INTERVAL_MS` - Poll interval in milliseconds (default: 3000)
/// - `RANDOMNESS_PROVIDER_ADDRESS` - Optional randomness provider address
#[derive(Clone)]
pub struct AppConfig {
    pub rpc_url: String,
    pub chain_id: u64,
    pub start_block: u64,
    /// PostgreSQL connection string (contains credentials - never log this)
    pub database_url: String,
    pub raffle_factory_address: String,
    pub randomness_provider_address: Option<String>,
    pub explorer_base_url: String,
    pub bind_addr: String,
    pub indexer_batch_size: u64,
    pub indexer_poll_interval_ms: u64,
}

// Implement Debug manually to avoid logging DATABASE_URL
impl std::fmt::Debug for AppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppConfig")
            .field("rpc_url", &self.rpc_url)
            .field("chain_id", &self.chain_id)
            .field("start_block", &self.start_block)
            .field("database_url", &"[REDACTED]")
            .field("raffle_factory_address", &self.raffle_factory_address)
            .field(
                "randomness_provider_address",
                &self.randomness_provider_address,
            )
            .field("explorer_base_url", &self.explorer_base_url)
            .field("bind_addr", &self.bind_addr)
            .field("indexer_batch_size", &self.indexer_batch_size)
            .field("indexer_poll_interval_ms", &self.indexer_poll_interval_ms)
            .finish()
    }
}

impl AppConfig {
    /// Loads configuration from environment variables
    ///
    /// # Errors
    /// Returns error if:
    /// - Required variables are missing (DATABASE_URL, RAFFLE_FACTORY_ADDRESS)
    /// - Numeric values fail to parse
    pub fn from_env() -> anyhow::Result<Self> {
        let rpc_url =
            env::var("RPC_URL").unwrap_or_else(|_| "https://rpc.testnet.arc.network".to_string());

        let chain_id = env::var("CHAIN_ID")
            .unwrap_or_else(|_| "5042002".to_string())
            .parse()
            .map_err(|_| anyhow::anyhow!("CHAIN_ID must be a valid u64"))?;

        let start_block = env::var("START_BLOCK")
            .unwrap_or_else(|_| "0".to_string())
            .parse()
            .map_err(|_| anyhow::anyhow!("START_BLOCK must be a valid u64"))?;

        // Required: DATABASE_URL (contains credentials)
        let database_url =
            env::var("DATABASE_URL").map_err(|_| anyhow::anyhow!("DATABASE_URL is required"))?;

        // Required: RAFFLE_FACTORY_ADDRESS
        let raffle_factory_address = env::var("RAFFLE_FACTORY_ADDRESS")
            .map_err(|_| anyhow::anyhow!("RAFFLE_FACTORY_ADDRESS is required"))?;

        // Validate address format (basic check)
        if !raffle_factory_address.starts_with("0x") || raffle_factory_address.len() != 42 {
            anyhow::bail!(
                "RAFFLE_FACTORY_ADDRESS must be a valid Ethereum address (0x + 40 hex chars)"
            );
        }

        let randomness_provider_address = env::var("RANDOMNESS_PROVIDER_ADDRESS").ok();

        let explorer_base_url = env::var("EXPLORER_BASE_URL")
            .unwrap_or_else(|_| "https://testnet.arcscan.app".to_string());

        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        let indexer_batch_size = env::var("INDEXER_BATCH_SIZE")
            .unwrap_or_else(|_| "2000".to_string())
            .parse()
            .map_err(|_| anyhow::anyhow!("INDEXER_BATCH_SIZE must be a valid u64"))?;

        let indexer_poll_interval_ms = env::var("INDEXER_POLL_INTERVAL_MS")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .map_err(|_| anyhow::anyhow!("INDEXER_POLL_INTERVAL_MS must be a valid u64"))?;

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
