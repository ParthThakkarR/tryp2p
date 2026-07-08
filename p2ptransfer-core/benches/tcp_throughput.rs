use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn bench_tcp_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("tcp/throughput");
    group.sample_size(20);

    for payload_size in [4096usize, 65536, 262144, 1048576] {
        let payload = vec![0xABu8; payload_size];
        group.throughput(criterion::Throughput::Bytes(payload_size as u64));

        group.bench_with_input(
            BenchmarkId::new("duplex", payload_size),
            &payload,
            |b, data| {
                b.to_async(&rt).iter(|| async {
                    // Use a fresh duplex each iteration — clean state
                    let (mut tx, rx) = tokio::io::duplex(2 * 1024 * 1024);

                    let reader = tokio::spawn(async move {
                        let mut len_buf = [0u8; 8];
                        let mut rx = rx;
                        rx.read_exact(&mut len_buf).await.unwrap();
                        let msg_len = u64::from_be_bytes(len_buf) as usize;
                        let mut buf = vec![0u8; msg_len];
                        rx.read_exact(&mut buf).await.unwrap();
                        black_box(buf);
                    });

                    tx.write_all(&(data.len() as u64).to_be_bytes())
                        .await
                        .unwrap();
                    tx.write_all(data).await.unwrap();
                    reader.await.unwrap();
                    black_box(())
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_tcp_throughput);
criterion_main!(benches);
