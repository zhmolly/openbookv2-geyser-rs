use anchor_lang::AnchorDeserialize;
use anchor_lang::Discriminator;
use base64::{prelude::BASE64_STANDARD, Engine};
use bytemuck::{self, cast_ref};
use openbook_v2::logs::{FillLog, OpenOrdersPositionLog, SettleFundsLog};
use openbook_v2::state::FillEvent;

use crate::structs::{Account, BotMsg, MessageTransaction, ObV2Cancel, ObV2Event, ObV2Fill};
use crate::utils::is_buy;
use crate::utils::token_decimals;
use crate::Parser;
use async_trait::async_trait;

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

        // No need to parse instructions (because we need generated events)
        /*
        // Loop instructions
        for (index, ix) in transaction.instructions.iter().enumerate() {
            if ix.program_id.eq(&self.program_id) && ix.accounts.contains(&account_idx) {
                // Check instruction type
                let discriminator = &ix.data[0..8];
                if discriminator == CancelOrder::DISCRIMINATOR {
                    tracing::info!("cancel order");

                    let tx = CancelOrder::deserialize(&mut &ix.data[8..])?;
                } else if discriminator == CancelAllAndPlaceOrders::DISCRIMINATOR {
                    tracing::info!("cancel & place orders");

                    let tx = CancelAllAndPlaceOrders::deserialize(&mut &ix.data[8..])?;
                } else if discriminator == PlaceOrder::DISCRIMINATOR {
                    tracing::info!("place order");

                    let tx = PlaceOrder::deserialize(&mut &ix.data[8..])?;
                } else if discriminator == PlaceTakeOrder::DISCRIMINATOR {
                    tracing::info!("place take order");

                    let tx = PlaceTakeOrder::deserialize(&mut &ix.data[8..])?;
                } else if discriminator == PlaceOrders::DISCRIMINATOR {
                    tracing::info!("place multiple orders");

                    let tx = PlaceOrders::deserialize(&mut &ix.data[8..])?;
                } else if discriminator == SettleFunds::DISCRIMINATOR {
                    tracing::info!("settle funds");
                } else if discriminator == ConsumeEvents::DISCRIMINATOR {
                    tracing::info!("consume events");
                } else {
                    tracing::info!("unknown obv2 ix");
                }
            }
        }
        */

        let mut events: Vec<ObV2Event> = vec![];
        let price_factor = token_decimals(self.base_decimals - self.quote_decimals)
            * self.quote_lot_size as f64
            / self.base_lot_size as f64;
        let base_factor = self.base_lot_size as f64 / token_decimals(self.base_decimals);

        // Check logs
        let mut start_idx: i16 = -1;
        for (idx, log) in transaction.logs.iter().enumerate() {
            // check openbookv2 events only
            if log.eq(&format!("Program {} invoke [1]", self.program_id)) {
                start_idx = idx as i16;
                continue;
            }

            // if openbookv2's ix finished
            if log.eq(&format!("Program {} success", self.program_id)) {
                start_idx = -1;
                continue;
            }

            // if in openbookv2 instruction
            if start_idx > -1 && log.starts_with("Program data:") {
                match BASE64_STANDARD.decode(&log[14..]) {
                    Ok(data) => {
                        // Check event type
                        let discriminator = &data[0..8];
                        if discriminator == FillLog::DISCRIMINATOR {
                            tracing::info!("fill order");

                            let fill = FillLog::deserialize(&mut &data[8..])?;

                            events.push(ObV2Event::Fill(ObV2Fill {
                                is_buy: fill.taker_side == 0,
                                taker: fill.taker.to_string(),
                                maker: fill.maker.to_string(),
                                order_id: fill.maker_client_order_id,
                                price: (fill.price as f64) * price_factor,
                                amount: fill.quantity as f64 * base_factor,
                            }));
                        } else if discriminator == OpenOrdersPositionLog::DISCRIMINATOR {
                            tracing::info!("positions order");
                        } else if discriminator == SettleFundsLog::DISCRIMINATOR {
                            // tracing::info!("settle funds");
                        } else {
                            tracing::info!("other event: {:?}", discriminator);
                        }
                    }
                    Err(_) => {
                        tracing::error!("Parse obv2 data error");
                    }
                }
            }
        }

        if events.len() > 0 {
            tracing::info!("total events: {:?}", events.len());
            Ok(BotMsg::ObV2Events(events))
        } else {
            Ok(BotMsg::Unimplemented)
        }
    }
}
