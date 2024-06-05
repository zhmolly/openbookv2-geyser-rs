use borsh::BorshDeserialize;
use itertools::Itertools;
use solana_sdk::{clock::UnixTimestamp, pubkey::Pubkey, signature::Signature};
use yellowstone_grpc_proto::{
    geyser::{
        SubscribeRequestFilterBlocksMeta, SubscribeUpdateAccount, SubscribeUpdateBlockMeta,
        SubscribeUpdateTransaction,
    },
    solana::storage::confirmed_block::{Message, TransactionStatusMeta},
};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Account {
    pub is_startup: bool,
    pub slot: u64,
    pub pubkey: Pubkey,
    pub lamports: u64,
    pub owner: Pubkey,
    pub executable: bool,
    pub rent_epoch: u64,
    pub data: Vec<u8>,
    pub write_version: u64,
    pub txn_signature: String,
}

impl From<SubscribeUpdateAccount> for Account {
    fn from(
        SubscribeUpdateAccount {
            is_startup,
            slot,
            account,
        }: SubscribeUpdateAccount,
    ) -> Self {
        let account = account.expect("should be defined");
        Self {
            is_startup,
            slot,
            pubkey: Pubkey::try_from(account.pubkey).expect("valid pubkey"),
            lamports: account.lamports,
            owner: Pubkey::try_from(account.owner).expect("valid pubkey"),
            executable: account.executable,
            rent_epoch: account.rent_epoch,
            data: account.data,
            write_version: account.write_version,
            txn_signature: bs58::encode(account.txn_signature.unwrap_or_default()).into_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MessageTransaction {
    pub signature: Signature,
    pub is_vote: bool,
    pub message: Message,
    pub meta: TransactionStatusMeta,
    pub index: u64,
    pub slot: u64,
}

impl From<SubscribeUpdateTransaction> for MessageTransaction {
    fn from(SubscribeUpdateTransaction { transaction, slot }: SubscribeUpdateTransaction) -> Self {
        let transaction = transaction.expect("transaction should be defined");
        let meta = transaction.meta.expect("meta should be defined");
        let tx_body = transaction.transaction.expect("tx body should be defined");
        let message = tx_body.message.expect("message should be defined");

        Self {
            signature: Signature::try_from(transaction.signature).unwrap_or_default(),
            is_vote: transaction.is_vote,
            message,
            meta,
            index: transaction.index,
            slot,
        }
    }
}

impl MessageTransaction {
    pub fn parse_message(
        &self,
        loaded_addresses: &[String],
    ) -> (Vec<String>, Vec<ParsedInstruction>) {
        let mut keys = self
            .message
            .account_keys
            .iter()
            .map(|pk| Pubkey::try_from_slice(pk).unwrap().to_string())
            .collect_vec();
        keys.extend_from_slice(loaded_addresses);
        let instructions = self
            .message
            .instructions
            .iter()
            .map(|instruction| ParsedInstruction {
                program_id: keys[instruction.program_id_index as usize].clone(),
                accounts: instruction.accounts.clone(),
                data: instruction.data.clone(),
            })
            .collect_vec();
        (keys, instructions)
    }

    pub fn to_parsed_transaction(&self) -> ParsedTransaction {
        let loaded_addresses = [
            self.meta
                .loaded_writable_addresses
                .iter()
                .map(|x| Pubkey::try_from_slice(x).unwrap().to_string())
                .collect_vec(),
            self.meta
                .loaded_readonly_addresses
                .iter()
                .map(|x| Pubkey::try_from_slice(x).unwrap().to_string())
                .collect_vec(),
        ]
        .concat();

        let (keys, instructions) = self.parse_message(&loaded_addresses);
        let is_err = self.meta.err.is_some();
        let logs = self.meta.log_messages.clone();

        let mut inner_instructions: Vec<Vec<ParsedInstruction>> = vec![];

        for (idx, ix) in self.message.instructions.iter().enumerate() {
            let inner_ixs = self
                .meta
                .inner_instructions
                .iter()
                .find(|t| t.index == (idx as u32));

            let inner_ixs = match inner_ixs {
                Some(inner_ixs) => inner_ixs
                    .instructions
                    .iter()
                    .map(|ii| ParsedInstruction {
                        program_id: keys[ii.program_id_index as usize].clone(),
                        accounts: ii.accounts.clone(),
                        data: ii.data.clone(),
                    })
                    .collect::<Vec<ParsedInstruction>>(),
                None => vec![],
            };

            inner_instructions.push(inner_ixs);
        }

        ParsedTransaction {
            slot: self.slot,
            block_time: None,
            signature: self.signature.to_string(),
            instructions,
            inner_instructions,
            accounts: keys,
            logs,
            is_err,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsedTransaction {
    pub slot: u64,
    pub block_time: Option<UnixTimestamp>,
    pub instructions: Vec<ParsedInstruction>,
    pub inner_instructions: Vec<Vec<ParsedInstruction>>,
    pub logs: Vec<String>,
    pub accounts: Vec<String>,
    pub is_err: bool,
    pub signature: String,
}

#[derive(Clone, Debug)]
pub struct ParsedInstruction {
    pub program_id: String,
    pub accounts: Vec<u8>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ParsedBlock {
    pub slot: u64,
    pub block_time: i64,
}

impl From<SubscribeUpdateBlockMeta> for ParsedBlock {
    fn from(
        SubscribeUpdateBlockMeta {
            slot, block_time, ..
        }: SubscribeUpdateBlockMeta,
    ) -> Self {
        let block_time = match block_time {
            Some(time) => time.timestamp,
            None => 0,
        };

        Self { slot, block_time }
    }
}

#[derive(Debug)]
pub struct OpenBook {
    pub owner: String,
    pub order_id: u128,
    pub is_buy: bool,
    pub price: f64,
    pub amount: f64,
}

#[derive(Debug)]
pub struct ObV2Fill {
    pub taker: String,
    pub maker: String,
    pub is_buy: bool,
    pub price: f64,
    pub amount: f64,
    pub order_id: u64,
}

#[derive(Debug)]
pub struct ObV2Cancel {
    pub seq_num: u64,
    pub owner: String,
    pub is_buy: bool,
    pub amount: f64,
}

#[derive(Debug)]
pub enum ObV2Event {
    Fill(ObV2Fill),
    Cancel(ObV2Cancel),
}

#[derive(Debug)]
pub struct ObV2BooksData {
    pub best: Option<f64>,
    pub books: Vec<OpenBook>,
}

#[derive(Debug)]
pub enum BotMsg {
    ObV2Books(ObV2BooksData),
    ObV2Events(Vec<ObV2Event>),
    Unimplemented,
}
