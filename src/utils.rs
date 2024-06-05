use anchor_lang::AnchorDeserialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub fn token_factor(decimals: u8) -> f64 {
    (10u64.pow(decimals as u32)) as f64
}

pub fn extract_transfer_amount(data: Vec<u8>) -> u64 {
    /*
    // first byte should 3 if transfer ix
    if data[0] != 3 {
        return 0;
    }
    */

    let amount: u64 = data[1..9]
        .iter()
        .rev()
        .fold(0, |acc, &x| acc * 256 + x as u64);
    amount
}

pub fn validate_ix_args<T>(data: &mut &Vec<u8>, ix_prefix: [u8; 8]) -> Option<T>
where
    T: AnchorDeserialize,
{
    let ix_header = data[..8].to_vec();
    if !ix_header.eq(&ix_prefix) {
        return None;
    }

    let ix_data = &mut data[8..].into();
    let data = AnchorDeserialize::deserialize(ix_data);
    if data.is_err() {
        return None;
    }

    Some(data.unwrap())
}

pub fn validate_ix_events<T>(data: &mut &Vec<u8>, ix_prefix: [u8; 16]) -> Option<T>
where
    T: AnchorDeserialize,
{
    let ix_header = data[..16].to_vec();
    if !ix_header.eq(&ix_prefix) {
        return None;
    }

    let ix_data = &mut data[16..].into();
    let data = AnchorDeserialize::deserialize(ix_data);
    if data.is_err() {
        return None;
    }

    Some(data.unwrap())
}

pub fn load_pubkey(data: [u64; 4]) -> Pubkey {
    let mut owner_bytes: [u8; 32] = [0; 32];
    for i in 0..4 {
        let bytes = data[i].to_le_bytes();
        owner_bytes[(i * 8)..(i * 8 + 8)].copy_from_slice(&bytes);
    }

    Pubkey::new_from_array(owner_bytes)
}

pub async fn get_ata(rpc_client: &RpcClient, ata: &str) -> anyhow::Result<()> {
    let account = rpc_client
        .get_account(&Pubkey::from_str(ata).unwrap())
        .await?;

    if account
        .owner
        .to_string()
        .eq("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
    {
        Ok(())
    } else {
        anyhow::bail!("Ata not exists");
    }
}

pub fn shortify_address(address: &str) -> String {
    if address.len() <= 120 {
        return address.to_string();
    }

    let prefix = &address[..4];
    let suffix = &address[(address.len() - 4)..];

    format!("{}...{}", prefix, suffix)
}
