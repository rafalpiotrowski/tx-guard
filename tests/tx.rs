use txp::{tx::TxProcessor, Transaction};
use tokio::sync::mpsc::{channel};
use std::io::{Cursor, BufRead, Write};

// #[tokio::test]
// #[cfg(target_os = "unix")]
// async fn process_transaction() {

//     let (tx_sender, tx_receiver) = channel::<Option<Transaction>>(2);


//     let t = Transaction { tx_type: txp::TxType::Deposit, client_id: 1, tx_id: 1, amount: 1.0, in_dispute: false };
//     tx_sender.send(Some(t)).await.expect("failed to send tx");
//     tx_sender.send(None).await.expect("failed to send None");

//     let mut c = Cursor::new(Vec::new());
//     let guard = StdoutOverride::override_file(c)?;

//     TxProcessor::process_transactions(tx_receiver, 2).await;

//     let mut out = Vec::new();
//     c.read_to_end(&mut out).unwrap();

//     let output = "client,available,held,total,locked\ndeposit,1,1,1.0000";

//     //todo test output
//     assert_eq!(output, out.);
// }