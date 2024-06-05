use std::cell::RefMut;
use std::mem;
use std::str::FromStr;
use bytemuck;

use crate::structs::{Account, BotMsg, OpenOrderData};
use crate::Extractor;
use anchor_lang::prelude::Pubkey;
use async_trait::async_trait;
use openbook_v2::state::{BookSide, Market};
use solana_client::nonblocking::rpc_client::RpcClient;

#[derive(Clone, Debug)]
pub struct ObV2AskPlugin {
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
impl Extractor for ObV2AskPlugin {
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

        let info = ObMarketInfo {
            base_decimals: self.base_decimals,
            base_lot_size: self.base_lot_size,
            quote_decimals: self.quote_decimals,
            quote_lot_size: self.quote_lot_size,
        };

        let mut best_ask = None;
        let mut asks = Vec::new();
        for l in leaves {
            let oid = l.key;
            let p = f64::from(readable_price(l.price() as f64, &info));
            let q = (l.quantity() as f64) / (info.quote_lot_size as f64);
            let owner = load_pubkey(l.owner);

            if best_ask.is_none() {
                best_ask = Some(p);
            }
            if owner.to_string() == self.oos_key {
                asks.push((oid, p, q));
            }
        }

        tracing::info!("best ask: {:?}", best_ask);

        Ok(BotMsg::ObV2Asks(OpenOrderData {
            best: None,
            open: vec![],
        }))
    }
}
