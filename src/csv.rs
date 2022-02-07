use std::path::PathBuf;

use futures::Future;
use tokio::fs::File;
use tokio_stream::StreamExt;

use tracing::{debug, error, trace};

use crate::{TxType, ClientId, Money, TxId};

/// Representation of the single row in the input CSV file
///
#[derive(serde::Deserialize, Debug)]
pub struct RawTransaction {
    #[serde(rename(deserialize = "type"))]
    pub tx_type: TxType,
    #[serde(rename(deserialize = "client"))]
    pub client_id: ClientId,
    #[serde(rename(deserialize = "tx"))]
    pub tx_id: TxId,

    // this at the moment does not work with csv_async library
    // parser raises error when no value is supplied
    //#[serde(rename(deserialize = "amount"), with = "rust_decimal::serde::float")]
    // amount: Money,
    #[serde(rename(deserialize = "amount"))]
    // work around to handle transactions types where amount is not specified
    pub amount: Option<String>,
}

#[derive(Serialize, Debug)]
pub(crate) struct RawAccount {
    #[serde(rename(deserialize = "client"))]
    pub client_id: ClientId,

    #[serde(rename(deserialize = "available"))]
    // The total funds that are available for trading, staking, withdrawal, etc. This
    // should be equal to the total - held amounts
    pub available_amount: Money,

    //#[serde(rename(deserialize = "held"), with = "rust_decimal::serde::str")]
    #[serde(rename(deserialize = "held"))]
    // The total funds that are held for dispute. This should be equal to total - available amounts
    pub held_amount: Money,

    #[serde(rename(deserialize = "total"))]
    // The total funds that are available or held. This should be equal to available + held
    pub total_amount: Money,

    #[serde(rename(deserialize = "locked"))]
    pub is_locked: bool,
}

pub struct CsvTransactionReader {}

impl CsvTransactionReader {

    /// Data processing function. Function calls panic! on the first error it gets.
    /// 
    /// 'data_file_path' full path to the file we want to process
    /// 'raw_transaction_handler' function that process the raw transaction
    pub async fn process_data_file<F, Fut>(
        data_file_path: PathBuf,
        raw_transaction_handler: F,
    ) 
    where
        F: Fn(Option<RawTransaction>) -> Fut,
        Fut: Future<Output = std::result::Result<(), String>>,
    {
        debug!("processing data file: {:?}", &data_file_path);

        let r = File::open(data_file_path).await;
        let file = match r {
            Ok(file) => file,
            Err(e) => {
                error!("failed opening data file: {}", e);
                panic!("failed opening data file: {e}");
            } 
        };

        trace!("data file opened; creating csv reader");

        let mut rdr = csv_async::AsyncReaderBuilder::new()
            .delimiter(b',')
            .flexible(true)
            .trim(csv_async::Trim::All)
            .has_headers(true)
            .create_deserializer(file);

        let mut records = rdr.deserialize::<RawTransaction>();

        while let Some(record) = records.next().await {
            match record {
                Ok(t) => {
                    trace!("processing raw transaction: {:?}", &t);
                    let r = raw_transaction_handler(Some(t)).await;
                    match r {
                        Ok(_) => continue,
                        Err(e) => {
                            error!("failed handling raw transaction: {}", e);
                            panic!("failed handling raw transaction: {e}");
                        }
                    }
                }
                Err(err) => {
                    error!("error reading CSV file: {}", err);
                    panic!("error reading CSV file: {err}");
                }
            }
        }

        debug!("all data processed from input file");

        let r = raw_transaction_handler(Option::None).await;
        match r {
            Ok(_) => (),
            Err(e) => {
                error!("failed to send end of data msg: {}", e);
                panic!("failed to send end of data msg: {e}");
            }
        }
        
        debug!("finished processing input file");
    }
}
