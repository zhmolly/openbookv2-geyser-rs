use bytemuck;
use std::mem;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::structs::{Account, BotMsg, ObV2BooksData, OpenBook};
use crate::utils::token_decimals;
use crate::Extractor;
use anchor_lang::prelude::Pubkey;
use async_trait::async_trait;
use openbook_v2::state::{BookSide, Side};
use solana_client::nonblocking::rpc_client::RpcClient;

#[derive(Clone, Debug)]
pub struct ObV2BooksPlugin {
    pub indicator_name: String,
    pub account: String,
    pub program_id: String,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}

#[async_trait]
impl Extractor for ObV2BooksPlugin {
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
        let data = &account.data;
        let bookside = bytemuck::from_bytes::<BookSide>(&data[8..mem::size_of::<BookSide>() + 8]);

        let is_buy = match bookside.side() {
            Side::Ask => false,
            Side::Bid => true,
        };
        let now_ts = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        let best_price = bookside.best_price(now_ts, None);
        let price_factor = token_decimals(self.base_decimals - self.quote_decimals)
            * self.quote_lot_size as f64
            / self.base_lot_size as f64;
        let base_factor = self.base_lot_size as f64 / token_decimals(self.base_decimals);

        let mut books: Vec<OpenBook> = vec![];

        bookside
            .iter_all_including_invalid(now_ts, None)
            .for_each(|order| {
                books.push(OpenBook {
                    order_id: order.node.key,
                    owner: order.node.owner.to_string(),
                    price: (order.price_lots as f64) * price_factor,
                    amount: (order.node.quantity as f64) * base_factor,
                    is_buy,
                });
            });

        let best = match best_price {
            Some(price) => Some((price as f64) * price_factor),
            None => None,
        };

        tracing::info!(
            "is_buy: {:?}, best: {:?}, books: {:?}",
            is_buy,
            best,
            books.len()
        );

        Ok(BotMsg::ObV2Books(ObV2BooksData { best, books }))
    }
}
