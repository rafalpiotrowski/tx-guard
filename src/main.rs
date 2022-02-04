use tokio::fs::File;
use tokio_stream::StreamExt;

#[macro_use]
extern crate serde;

type ClientId = u16;
type TxId = u32;
type Amount = rust_decimal::Decimal;

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "lowercase")]
enum TxType {
    Deposit,
    Withdrawal,
    Dispiute,
    Resolve,
    Chargeback,
}

#[derive(Deserialize, Serialize, Debug)]
struct Transaction {
    #[serde(rename(deserialize = "type"))]
    tx_type: TxType,
    #[serde(rename(deserialize = "client"))]
    client_id: ClientId,
    #[serde(rename(deserialize = "tx"))]
    tx_id: TxId,
    #[serde(rename(deserialize = "amount"), with = "rust_decimal::serde::float")]
    amount: Amount,
}

#[derive(Serialize, Debug)]
struct Account {
    #[serde(rename(deserialize = "client"))]
    client_id: ClientId,
    #[serde(rename(deserialize = "available"), with = "rust_decimal::serde::str")]
    available_amount: Amount,
    #[serde(rename(deserialize = "held"), with = "rust_decimal::serde::str")]
    held_amount: Amount,
    #[serde(rename(deserialize = "total"), with = "rust_decimal::serde::str")]
    total_amount: Amount,
    #[serde(rename(deserialize = "locked"))]
    is_locked: bool,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {

    let args: Vec<String> = std::env::args().collect();
    let data_file_path = &args[1];

    let mut rdr = csv_async::AsyncReaderBuilder::new()
        .delimiter(b',')
        .trim(csv_async::Trim::All)
        .has_headers(true)
        .create_deserializer(File::open(data_file_path).await?);

    let mut records = rdr.deserialize::<Transaction>();
    while let Some(record) = records.next().await {
        let record = record?;
        println!("{:?}", record);
    }
    Ok(())
}
