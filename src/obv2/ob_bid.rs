use std::str::FromStr;

use crate::structs::{Account, BotMsg, OpenOrderData};
use crate::Extractor;
use async_trait::async_trait;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;

#[derive(Clone, Debug)]
pub struct ObV2BidPlugin {
    pub oos_key: String,
    pub indicator_name: String,
    pub account: String,
    pub program_id: String,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}

#[async_trait]
impl Extractor for ObV2BidPlugin {
    fn name(&self) -> String {
        self.indicator_name.clone()
    }

    fn program_id(&self) -> String {
        self.program_id.clone()
    }
    fn account(&self) -> String {
        self.account.clone()
    }

    async fn load(&mut self, client: &RpcClient) -> anyhow::Result<BotMsg> {
        let account_pubkey = Pubkey::from_str(&self.account).unwrap();
        let account = client.get_account(&account_pubkey).await;
        if account.is_ok() {
            let account = account.unwrap();
            return self.extract(&mut Account {
                is_startup: false,
                slot: 0,
                pubkey: account_pubkey,
                lamports: account.lamports,
                owner: account.owner,
                executable: account.executable,
                rent_epoch: account.rent_epoch,
                data: account.data,
                write_version: 0,
                txn_signature: String::new(),
            });
        }

        Ok(BotMsg::Unimplemented)
    }

    fn extract(&mut self, account: &mut Account) -> anyhow::Result<BotMsg> {
        let data = account.data;
        let books: RefMut<BookSide> =
            bytemuck::from_bytes_mut(&mut data[8..mem::size_of::<BookSide>() + 8]);

        let leaves = data.traverse(true);

        let info = ObMarketInfo {
            base_decimals: self.base_decimals,
            base_lot_size: self.base_lot_size,
            quote_decimals: self.quote_decimals,
            quote_lot_size: self.quote_lot_size,
        };

        let mut best_bid = None;
        let mut bids = Vec::new();
        for l in leaves {
            let oid = l.key;
            let p = f64::from(readable_price(l.price() as f64, &info));
            let q = (l.quantity() as f64) / (info.quote_lot_size as f64);
            let owner = load_pubkey(l.owner);
            if best_bid.is_none() {
                best_bid = Some(p);
            }
            if owner.to_string() == self.oos_key {
                bids.push((oid, p, q));
            }
        }

        tracing::info!("best bid: {:?}", best_bid);

        Ok(BotMsg::ObV1Bids(OpenOrderData {
            best: best_bid,
            open: bids,
        }))

        Ok(BotMsg::ObV2Bids(OpenOrderData {
            best: None,
            open: vec![],
        }))
    }
}
