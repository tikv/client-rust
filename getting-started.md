# Getting started

The TiKV client is a Rust library (crate). To use this crate in your project, add the following dependencies to your `Cargo.toml`:

```toml
[dependencies]
tikv-client = "0.1"
tokio = { version = "1.5", features = ["full"] }
```

Note that you need to use Tokio. The TiKV client has an async API and therefore you need an async runtime in your program to use it. At the moment, Tokio is used internally in the client and so you must use Tokio in your code too. We plan to become more flexible in future versions.

The minimum supported version of Rust is 1.40. The minimum supported version of TiKV is 5.0.

The general flow of using the client crate is to create either a raw or transaction client object (which can be configured) then send commands using the client object, or use it to create transactions objects. In the latter case, the transaction is built up using various commands and then committed (or rolled back).

## Examples

To use the client in your program, use code like the following.

Raw mode:

```rust
use tikv_client::RawClient;

let client = RawClient::new(vec!["127.0.0.1:2379"]).await?;
client.put("key".to_owned(), "value".to_owned()).await?;
let value = client.get("key".to_owned()).await?;
```

Transactional mode:

```rust
use tikv_client::TransactionClient;

let txn_client = TransactionClient::new(vec!["127.0.0.1:2379"]).await?;
let mut txn = txn_client.begin_optimistic().await?;
txn.put("key".to_owned(), "value".to_owned()).await?;
let value = txn.get("key".to_owned()).await?;
txn.commit().await?;
```

To make an example which builds and runs,

```rust
use tikv_client::{TransactionClient, Error};

async fn run() -> Result<(), Error> {
    let txn_client = TransactionClient::new(vec!["127.0.0.1:2379"]).await?;
    let mut txn = txn_client.begin_optimistic().await?;
    txn.put("key".to_owned(), "value".to_owned()).await?;
    let value = txn.get("key".to_owned()).await?;
    println!("value: {:?}", value);
    txn.commit().await?;
    Ok(())
}

#[tokio::main]
async fn main() {
    run().await.unwrap();
}
```

For more examples, see the [examples](examples) directory.
