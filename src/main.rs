pub mod obv2;
pub mod structs;
pub mod subscribe;
pub mod utils;

use crate::structs::{Account, MessageTransaction};
use crate::subscribe::subscribe_geyser;
use async_trait::async_trait;
use obv2::{ObV2AskPlugin, ObV2BidPlugin};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::env;
use std::sync::Arc;
use structs::BotMsg;
use tokio::task::JoinHandle;

#[async_trait]
pub trait Extractor: Send + Sync {
    fn name(&self) -> String;

    fn program_id(&self) -> String;

    fn account(&self) -> String;

    fn extract(&mut self, account: &mut Account) -> anyhow::Result<BotMsg>;

    async fn load(&mut self, client: &RpcClient) -> anyhow::Result<BotMsg>;
}

pub trait Parser: Send + Sync {
    fn name(&self) -> String;

    fn program_id(&self) -> String;

    fn account(&self) -> String;

    fn parse(&self, transaction: &MessageTransaction) -> anyhow::Result<BotMsg>;
}

#[tokio::main()]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let rpc_url = env::var("RPC_URL").expect("RPC_URL not set in .env");
    let triton_url = env::var("TRITON_URL").expect("TRITON_URL not set in .env");
    let triton_token = env::var("TRITON_TOKEN").expect("TRITON_TOKEN not set in .env");

    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_file(false)
        // .with_target(false)
        // .without_time()
        .init();

    let mut parsers = Vec::new();
    let mut extractors: Vec<Box<dyn Extractor>> = Vec::new();

    extractors.push(Box::new(ObV2AskPlugin {
        indicator_name: "ob_v2_sol_usdc_asks".to_string(),
        account: "53v47CBoaKwoM8tSEDN4oNyCc2ZJenDeuhMJTEw7fL2M".to_string(),
        program_id: "opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb".to_string(),
        oos_key: "FdrprVmTtB8ymE4RQ7ugYHQtDhDy6hDhpDviWEuuvMtj".to_string(),
        base_decimals: 9,
        quote_decimals: 6,
        base_lot_size: 1000000,
        quote_lot_size: 1,
    }));

    extractors.push(Box::new(ObV2BidPlugin {
        indicator_name: "ob_v2_sol_usdc_bids".to_string(),
        account: "Ad5skEiFoaeA27G3UhbpuwnFBCvmuuGEyoiijZhcd5xX".to_string(),
        program_id: "opnb2LAfJYbRMAHHvqjCwQxanZn7ReEHp1k81EohpZb".to_string(),
        oos_key: "FdrprVmTtB8ymE4RQ7ugYHQtDhDy6hDhpDviWEuuvMtj".to_string(),
        base_decimals: 9,
        quote_decimals: 6,
        base_lot_size: 1000000,
        quote_lot_size: 1,
    }));

    // subscribe geyser with extractor accounts
    loop {
        match subscribe_geyser(
            rpc_url.clone(),
            triton_url.clone(),
            triton_token.clone(),
            &mut extractors,
            &parsers,
        )
        .await
        {
            Ok(()) => {
                tracing::info!("Geyser subscribe finished");
            }
            Err(e) => {
                tracing::error!("Subscribe geyser failed: {:?}", e);
            }
        };
    }
}
