extern crate futures;
extern crate tikv_client;

use futures::{Async, Future, Stream};
use tikv_client::transaction::{Client, Mutator, Retriever, TxnClient};
use tikv_client::*;

fn puts<P, I>(client: &TxnClient, pairs: P)
where
    P: IntoIterator<Item = I>,
    I: Into<KvPair>,
{
    let mut txn = client.begin().wait().expect("Could not begin transaction");
    let _: Vec<()> = pairs
        .into_iter()
        .map(Into::into)
        .map(|p| {
            txn.set(p).wait().expect("Could not set key value pair");
        }).collect();
    txn.commit().wait().expect("Could not commit transaction");
}

fn get(client: &TxnClient, key: &Key) -> Value {
    let txn = client.begin().wait().expect("Could not begin transaction");
    txn.get(key).wait().expect("Could not get value")
}

fn scan(client: &TxnClient, start: &Key, limit: usize) {
    let txn = client.begin().wait().expect("Could not begin transaction");
    let mut scanner = txn.seek(start).wait().expect("Could not seek to start key");
    let mut limit = limit;
    loop {
        if limit == 0 {
            break;
        }
        match scanner.poll() {
            Ok(Async::Ready(None)) => return,
            Ok(Async::Ready(Some(pair))) => {
                limit -= 1;
                println!("{:?}", pair);
            }
            _ => break,
        }
    }
}

fn dels<P>(client: &TxnClient, pairs: P)
where
    P: IntoIterator<Item = Key>,
{
    let mut txn = client.begin().wait().expect("Could not begin transaction");
    let _: Vec<()> = pairs
        .into_iter()
        .map(|p| {
            txn.delete(p).wait().expect("Could not delete key");
        }).collect();
    txn.commit().wait().expect("Could not commit transaction");
}

fn main() {
    let config = Config::new(vec!["127.0.0.1:3379"]);
    let txn = TxnClient::new(&config)
        .wait()
        .expect("Could not connect to tikv");

    // set
    let key1: Key = b"key1".to_vec().into();
    let value1: Value = b"value1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    let value2: Value = b"value2".to_vec().into();
    puts(&txn, vec![(key1, value1), (key2, value2)]);

    // get
    let key1: Key = b"key1".to_vec().into();
    let value1 = get(&txn, &key1);
    println!("{:?}", (key1, value1));

    // scan
    let key1: Key = b"key1".to_vec().into();
    scan(&txn, &key1, 10);

    // delete
    let key1: Key = b"key1".to_vec().into();
    let key2: Key = b"key2".to_vec().into();
    dels(&txn, vec![key1, key2]);
}
