use crate::{arb_batch, generate, pd_addrs};
use criterion::{black_box, criterion_group, BatchSize, Bencher, Criterion};
use futures::executor::block_on;
use proptest::arbitrary::any;
use tikv_client::{raw::Client, Config, KvPair, Result};

fn client() -> Result<Client> {
    let connect = Client::connect(Config::new(pd_addrs()));
    block_on(connect)
}

criterion_group!(suite, scenarios);

pub fn scenarios(c: &mut Criterion) {
    c.bench_function("raw::put", move |b| {
        put(b).unwrap();
    });
    c.bench_function("raw::get", move |b| {
        get(b).unwrap();
    });
    c.bench_function("raw::delete", move |b| {
        delete(b).unwrap();
    });
    c.bench_function("raw::batch_put", move |b| {
        batch_put(b).unwrap();
    });
    c.bench_function("raw::batch_get", move |b| {
        batch_get(b).unwrap();
    });
    c.bench_function("raw::batch_delete", move |b| {
        batch_delete(b).unwrap();
    });
}

fn put(b: &mut Bencher) -> Result<()> {
    let client = client()?;
    let mut cleanup_keys = Vec::new();
    b.iter_batched(
        || {
            let kv = generate(any::<KvPair>());
            cleanup_keys.push(kv.key().clone());
            kv
        },
        |kv| {
            let res = block_on(client.put(kv.key().clone(), kv.value().clone())).unwrap();
            black_box(res)
        },
        BatchSize::PerIteration,
    );
    block_on(client.batch_delete(cleanup_keys)).unwrap();
    Ok(())
}

fn get(b: &mut Bencher) -> Result<()> {
    let client = client()?;
    let mut cleanup_keys = Vec::new();
    b.iter_batched(
        || {
            let kv = generate(any::<KvPair>());
            block_on(client.put(kv.key().clone(), kv.value().clone())).unwrap();
            cleanup_keys.push(kv.key().clone());
            kv
        },
        |kv| {
            let res = block_on(client.get(kv.key().clone())).unwrap();
            black_box(res)
        },
        BatchSize::PerIteration,
    );
    block_on(client.batch_delete(cleanup_keys)).unwrap();
    Ok(())
}

fn delete(b: &mut Bencher) -> Result<()> {
    let client = client()?;
    b.iter_batched(
        || {
            let kv = generate(any::<KvPair>());
            block_on(client.put(kv.key().clone(), kv.value().clone())).unwrap();
            kv
        },
        |kv| {
            let res = block_on(client.delete(kv.key().clone())).unwrap();
            black_box(res)
        },
        BatchSize::PerIteration,
    );
    Ok(())
}

fn batch_put(b: &mut Bencher) -> Result<()> {
    let client = client()?;
    let mut cleanup_keys = Vec::new();
    b.iter_batched(
        || {
            let kvs = generate(arb_batch(any::<KvPair>(), None));
            let keys = kvs
                .clone()
                .into_iter()
                .map(|v| v.into_key())
                .collect::<Vec<_>>();
            cleanup_keys.append(&mut keys.clone());
            kvs
        },
        |kvs| {
            let res = block_on(client.batch_put(kvs)).unwrap();
            black_box(res)
        },
        BatchSize::PerIteration,
    );
    block_on(client.batch_delete(cleanup_keys)).unwrap();
    Ok(())
}

fn batch_get(b: &mut Bencher) -> Result<()> {
    let client = client()?;
    let mut cleanup_keys = Vec::new();
    b.iter_batched(
        || {
            let kvs = generate(arb_batch(any::<KvPair>(), None));
            block_on(client.batch_put(kvs.clone())).unwrap();
            let keys = kvs
                .clone()
                .into_iter()
                .map(|v| v.into_key())
                .collect::<Vec<_>>();
            cleanup_keys.append(&mut keys.clone());
            keys
        },
        |keys| {
            let res = block_on(client.batch_get(keys)).unwrap();
            black_box(res)
        },
        BatchSize::PerIteration,
    );
    block_on(client.batch_delete(cleanup_keys)).unwrap();
    Ok(())
}

fn batch_delete(b: &mut Bencher) -> Result<()> {
    let client = client()?;
    b.iter_batched(
        || {
            let kvs = generate(arb_batch(any::<KvPair>(), None));
            block_on(client.batch_put(kvs.clone())).unwrap();
            let keys = kvs.into_iter().map(|v| v.into_key()).collect::<Vec<_>>();
            keys
        },
        |keys| {
            let res = block_on(client.batch_delete(keys)).unwrap();
            black_box(res)
        },
        BatchSize::PerIteration,
    );
    Ok(())
}
