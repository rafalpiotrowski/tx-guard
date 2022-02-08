use std::path::PathBuf;

use txp::{csv::{CsvTransactionReader, RawTransaction}, Transaction};

/// basic test to check if working
#[tokio::test]
async fn read_csv_file_to_the_end() {

    let mut data_file_path = std::path::PathBuf::new();
    data_file_path.push("tests/transactions.csv");

    dummy_read(data_file_path).await;

    assert_eq!(0, 0);
}

#[tokio::test]
#[should_panic]
async fn unknown_type_in_data_file() {
    let mut data_file_path = std::path::PathBuf::new();
    data_file_path.push("tests/transactions_wrong_type.csv");

    dummy_read(data_file_path).await;

    assert_eq!(0, 0);
}

#[tokio::test]
#[should_panic]
async fn wrong_client_id_type() {
    let mut data_file_path = std::path::PathBuf::new();
    data_file_path.push("tests/transactions_wrong_client_id_type.csv");

    dummy_read(data_file_path).await;

    assert_eq!(0, 0);
}

#[tokio::test]
#[should_panic]
async fn wrong_tx_id_type() {
    let mut data_file_path = std::path::PathBuf::new();
    data_file_path.push("tests/transactions_wrong_tx_id_type.csv");

    dummy_read(data_file_path).await;

    assert_eq!(0, 0);
}

#[tokio::test]
#[should_panic]
async fn wrong_amount_type() {
    let mut data_file_path = std::path::PathBuf::new();
    data_file_path.push("tests/transactions_wrong_amount_type.csv");

    dummy_read(data_file_path).await;

    assert_eq!(0, 0);
}

#[tokio::test]
#[should_panic]
async fn non_exisiting_data_file() {
    let mut data_file_path = std::path::PathBuf::new();
    data_file_path.push("tests/nonexisintg_file.csv");

    dummy_read(data_file_path).await;

    assert_eq!(0, 0);
}

async fn dummy_read(data_file_path: PathBuf)
{
    let raw_transaction_handler = |rt: Option<RawTransaction>| async move {
        // dummy handler
        match rt {
            Some(rt) => {
                let t: Transaction = rt.into();
                print!("{:?}", t);
            }
            None => print!("EOF")
        }
        
        Ok(())
    };
    let _reader = CsvTransactionReader::process_data_file(data_file_path, raw_transaction_handler).await;
}