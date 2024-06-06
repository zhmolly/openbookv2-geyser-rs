use bytemuck::{self, cast_ref};
use std::mem;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::structs::{Account, BotMsg, ObV2Cancel, ObV2Event, ObV2Fill};
use crate::utils::{is_buy, token_decimals};
use crate::Extractor;
use anchor_lang::prelude::Pubkey;
use async_trait::async_trait;
use openbook_v2::state::{BookSide, EventHeap, EventType, FillEvent, OutEvent, Side};
use solana_client::nonblocking::rpc_client::RpcClient;

#[derive(Clone, Debug)]
pub struct ObV2EventsPlugin {
    pub indicator_name: String,
    pub account: String,
    pub program_id: String,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}

#[async_trait]
impl Extractor for ObV2EventsPlugin {
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
        let event_heap =
            bytemuck::from_bytes::<EventHeap>(&data[8..mem::size_of::<EventHeap>() + 8]);

        let mut events: Vec<ObV2Event> = vec![];

        let price_factor = token_decimals(self.base_decimals - self.quote_decimals)
            * self.quote_lot_size as f64
            / self.base_lot_size as f64;
        let base_factor = self.base_lot_size as f64 / token_decimals(self.base_decimals);

        for node in event_heap.nodes {
            if node.is_free() {
                continue;
            }

            let event = node.event;

            match EventType::try_from(event.event_type).unwrap() {
                EventType::Fill => {
                    let fill: &FillEvent = cast_ref(&event);
                    events.push(ObV2Event::Fill(ObV2Fill {
                        is_buy: is_buy(fill.taker_side()),
                        taker: fill.taker.to_string(),
                        maker: fill.maker.to_string(),
                        order_id: fill.maker_client_order_id,
                        price: (fill.price as f64) * price_factor,
                        amount: fill.quantity as f64 * base_factor,
                    }));
                }
                EventType::Out => {
                    let out: &OutEvent = cast_ref(&event);
                    events.push(ObV2Event::Cancel(ObV2Cancel {
                        is_buy: is_buy(out.side()),
                        owner: out.owner.to_string(),
                        seq_num: out.seq_num,
                        amount: out.quantity as f64 * base_factor,
                    }));
                }
            }
        }

        tracing::info!("total events: {:?}", events.len());

        Ok(BotMsg::ObV2Events(events))
    }
}
