use anchor_lang::{AnchorDeserialize, Discriminator};
use bytemuck::{self, cast_ref};
use openbook_v2::instruction::{
    CancelAllAndPlaceOrders, CancelOrder, ConsumeEvents, PlaceOrder, PlaceOrders, PlaceTakeOrder,
    SettleFunds,
};
use std::mem;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::structs::{Account, BotMsg, MessageTransaction, ObV2Cancel, ObV2Event, ObV2Fill};
use crate::utils::{is_buy, token_decimals};
use crate::Parser;
use anchor_lang::prelude::Pubkey;
use async_trait::async_trait;
use openbook_v2::state::{BookSide, EventHeap, EventType, FillEvent, OutEvent, Side};
use solana_client::nonblocking::rpc_client::RpcClient;

#[derive(Clone, Debug)]
pub struct ObV2TransactionsPlugin {
    pub indicator_name: String,
    pub account: String,
    pub program_id: String,
    pub base_lot_size: u64,
    pub quote_lot_size: u64,
    pub base_decimals: u8,
    pub quote_decimals: u8,
}

#[async_trait]
impl Parser for ObV2TransactionsPlugin {
    fn name(&self) -> String {
        self.indicator_name.clone()
    }

    fn program_id(&self) -> String {
        self.program_id.clone()
    }
    fn account(&self) -> String {
        self.account.clone()
    }

    fn parse(&self, tx: &MessageTransaction) -> anyhow::Result<BotMsg> {
        let slot = tx.slot;

        let transaction = tx.to_parsed_transaction();
        let account_idx = transaction
            .accounts
            .iter()
            .position(|t| t.eq(&self.account))
            .unwrap() as u8;

        tracing::info!("tx: {}, slot: {}", transaction.signature, slot);

        // Loop instructions
        for (index, ix) in transaction.instructions.iter().enumerate() {
            if ix.program_id.eq(&self.program_id) && ix.accounts.contains(&account_idx) {
                // Check instruction type
                let discriminator = &ix.data[0..8];
                if discriminator == CancelOrder::DISCRIMINATOR {
                    tracing::info!("cancel order");
                } else if discriminator == CancelAllAndPlaceOrders::DISCRIMINATOR {
                    tracing::info!("cancel & place orders");

                    let tx = CancelAllAndPlaceOrders::deserialize(&mut &ix.data[8..])?;
                    tracing::info!("bids: {:?}", tx.bids);
                    tracing::info!("asks: {:?}", tx.asks);
                } else if discriminator == PlaceOrder::DISCRIMINATOR {
                    tracing::info!("place order");
                } else if discriminator == PlaceTakeOrder::DISCRIMINATOR {
                    tracing::info!("place take order");
                } else if discriminator == PlaceOrders::DISCRIMINATOR {
                    tracing::info!("place multiple orders");
                } else if discriminator == SettleFunds::DISCRIMINATOR {
                    tracing::info!("settle funds");
                } else if discriminator == ConsumeEvents::DISCRIMINATOR {
                    tracing::info!("consume events");
                } else {
                    tracing::info!("unknown obv2 ix");
                }
            }
        }

        Ok(BotMsg::Unimplemented)
    }
}
