use txp::{tx::TxProcessor, Transaction};
use tokio::sync::mpsc::{channel};
use stdio_override::StdoutOverride;

#[tokio::test]
#[cfg(target_family = "unix")]
async fn process_transaction() {
    use std::fs;

    let (tx_sender, tx_receiver) = channel::<Option<Transaction>>(2);

    let t = Transaction { tx_type: txp::TxType::Deposit, client_id: 1, tx_id: 1, amount: 1.0, in_dispute: false };
    tx_sender.send(Some(t)).await.expect("failed to send tx");
    tx_sender.send(None).await.expect("failed to send None");

    let file_name = "./test_stdout.txt";
    let _guard = StdoutOverride::override_file(file_name).expect("faild to redirect stdout");

    TxProcessor::process_transactions(tx_receiver, 2).await;

    let captured_stdout = fs::read_to_string(file_name).expect("failed to captured stdout file content");

    fs::remove_file(file_name).expect("failed to remove file");

    let expected_output = "1,1.0000,0.0000,1.0000,false\n".to_string();

    assert_eq!(captured_stdout, expected_output);
}