use super::Extractor;
use super::Parser;
use crate::structs::ParsedBlock;
use crate::structs::{Account, MessageTransaction};
use futures::{sink::SinkExt, stream::StreamExt};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};
use tokio::time::{interval, timeout};
use yellowstone_grpc_client::{GeyserGrpcClient, GeyserGrpcClientError};
use yellowstone_grpc_proto::geyser::SubscribeRequestFilterBlocksMeta;
use yellowstone_grpc_proto::geyser::SubscribeRequestPing;
// use structs::response_data::IndicatorData;
use yellowstone_grpc_proto::prelude::{
    subscribe_update::UpdateOneof, CommitmentLevel, SubscribeRequest,
    SubscribeRequestFilterAccounts, SubscribeRequestFilterTransactions,
};

type AccountsFilterMap = HashMap<String, SubscribeRequestFilterAccounts>;
type TransactionsFilterMap = HashMap<String, SubscribeRequestFilterTransactions>;

pub fn unique_array(arr: Vec<String>) -> Vec<String> {
    let hashset: HashSet<String> = arr.into_iter().collect();
    hashset.into_iter().collect()
}

pub async fn subscribe_geyser(
    rpc_url: String,
    triton_url: String,
    triton_token: String,
    extractors: &mut Vec<Box<dyn Extractor>>,
    parsers: &Vec<Box<dyn Parser>>,
) -> anyhow::Result<()> {
    // Load initial state for extractors
    let client = RpcClient::new(rpc_url);
    for extractor in extractors.iter_mut() {
        match extractor.load(&client).await {
            Ok(data) => {
                // tracing::info!("{:?}", data);
            }
            Err(_) => {}
        }
    }

    // Connect geyser client
    let mut geyser_client = GeyserGrpcClient::build_from_shared(triton_url)?
        .x_token(Some(triton_token))?
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(10))
        .connect()
        .await
        .map_err(|e| tracing::error!("Geyser error: {:?}", e))
        .expect("failed to connect geyser");
    tracing::info!("Connected to geyser...");

    // prepare subscribe filter
    let mut request = SubscribeRequest::default();

    let mut accounts_filter: AccountsFilterMap = HashMap::new();
    for extractor in extractors.iter() {
        accounts_filter.insert(
            extractor.name(),
            SubscribeRequestFilterAccounts {
                account: vec![extractor.account()],
                owner: vec![extractor.program_id()],
                filters: [].into(),
            },
        );
    }
    request.accounts = accounts_filter;

    let mut transaction_filter: TransactionsFilterMap = HashMap::new();
    if parsers.len() > 0 {
        for parser in parsers.iter() {
            transaction_filter.insert(
                parser.name(),
                SubscribeRequestFilterTransactions {
                    vote: None,
                    failed: Some(false),
                    signature: None,
                    account_include: vec![parser.account()],
                    account_exclude: vec![],
                    account_required: vec![parser.program_id()],
                },
            );
        }
        request.transactions = transaction_filter;
    }

    // Get only confirmed status
    request.set_commitment(CommitmentLevel::Confirmed);

    let (mut subscribe_tx, mut stream) = geyser_client.subscribe().await.unwrap();
    subscribe_tx
        .send(request.clone())
        .await
        .map_err(GeyserGrpcClientError::SubscribeSendError)
        .unwrap();

    let _ = futures::try_join!(
        async move {
            // Setup ping timer for every 10 seconds
            let mut timer = interval(Duration::from_secs(10));
            let mut id = 0;
            loop {
                timer.tick().await;
                id += 1;
                subscribe_tx
                    .send(SubscribeRequest {
                        ping: Some(SubscribeRequestPing { id }),
                        ..Default::default()
                    })
                    .await?;
            }
            #[allow(unreachable_code)]
            Ok::<(), anyhow::Error>(())
        },
        async move {
            loop {
                let mut is_received = false;
                match timeout(Duration::from_secs(10), stream.next()).await {
                    Ok(Some(message)) => {
                        is_received = true;

                        match message {
                            Ok(msg) => {
                                #[allow(clippy::single_match)]
                                #[allow(clippy::multiple_unsafe_ops_per_block)]
                                match msg.update_oneof {
                                    Some(UpdateOneof::Account(account)) => {
                                        // It can be multi filter
                                        let mut account: Account = account.into();

                                        for filter in msg.filters {
                                            match extractors
                                                .iter_mut()
                                                .find(|t| t.name().eq(&filter))
                                            {
                                                Some(extractor) => {
                                                    match extractor.extract(&mut account) {
                                                        Ok(data) => {
                                                            // tracing::info!("{:?}", data);
                                                        }
                                                        Err(e) => {
                                                            tracing::error!(
                                                                "Subscribe account error: {}",
                                                                e
                                                            )
                                                        }
                                                    }
                                                }
                                                None => {}
                                            };
                                        }
                                    }
                                    Some(UpdateOneof::Transaction(transaction)) => {
                                        let transaction: MessageTransaction = transaction.into();

                                        for filter in msg.filters {
                                            match parsers
                                                .iter()
                                                .find(|t| t.name().eq(&filter))
                                                .unwrap()
                                                .parse(&transaction)
                                            {
                                                Ok(data) => {
                                                    // tracing::info!("{:?}", data);
                                                }
                                                Err(e) => {
                                                    tracing::info!("Subscribe error: {}", e)
                                                }
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Err(e) => {
                                // tracing::info!("Subscribe parse error: {:?}", e);
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        tracing::error!("Subscribe geyser error: {:?}", e);
                    }
                }

                if !is_received {
                    break;
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
            }

            Ok::<(), anyhow::Error>(())
        }
    );

    tracing::info!("Subscribe geyser finished");
    Ok(())
}
