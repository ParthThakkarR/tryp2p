use criterion::{Criterion, black_box, criterion_group, criterion_main};

fn bench_zstd_compress(c: &mut Criterion) {
    let data = vec![0xABu8; 1024 * 1024]; // 1MB
    let mut group = c.benchmark_group("compression/zstd");
    group.sample_size(20);

    group.bench_function("compress_1mb", |b| {
        b.iter(|| {
            let compressed = zstd::encode_all(std::io::Cursor::new(&data), 3).unwrap();
            black_box(compressed)
        })
    });

    let compressed = zstd::encode_all(std::io::Cursor::new(&data), 3).unwrap();
    group.bench_function("decompress_1mb", |b| {
        b.iter(|| {
            let decompressed = zstd::decode_all(std::io::Cursor::new(&compressed)).unwrap();
            black_box(decompressed)
        })
    });
    group.finish();
}

fn bench_lz4_compress(c: &mut Criterion) {
    let data = vec![0xABu8; 1024 * 1024]; // 1MB
    let mut group = c.benchmark_group("compression/lz4");
    group.sample_size(20);

    group.bench_function("compress_1mb", |b| {
        b.iter(|| {
            let compressed = lz4_flex::compress_prepend_size(&data);
            black_box(compressed)
        })
    });

    let compressed = lz4_flex::compress_prepend_size(&data);
    group.bench_function("decompress_1mb", |b| {
        b.iter(|| {
            let decompressed = lz4_flex::decompress_size_prepended(&compressed).unwrap();
            black_box(decompressed)
        })
    });
    group.finish();
}

fn bench_compression_ratio(c: &mut Criterion) {
    let data = vec![0xABu8; 1024 * 1024]; // repetitive → compressible
    let mut group = c.benchmark_group("compression/ratio");

    let zstd_len = zstd::encode_all(std::io::Cursor::new(&data), 3)
        .unwrap()
        .len();
    let lz4_len = lz4_flex::compress_prepend_size(&data).len();

    group.bench_function("zstd_ratio", |b| {
        b.iter(|| {
            let ratio = zstd_len as f64 / data.len() as f64;
            black_box(ratio)
        })
    });

    group.bench_function("lz4_ratio", |b| {
        b.iter(|| {
            let ratio = lz4_len as f64 / data.len() as f64;
            black_box(ratio)
        })
    });

    println!(
        "Compression ratios (repetitive 1MB): zstd={:.3}, lz4={:.3}",
        zstd_len as f64 / data.len() as f64,
        lz4_len as f64 / data.len() as f64,
    );
    group.finish();
}

criterion_group!(
    benches,
    bench_zstd_compress,
    bench_lz4_compress,
    bench_compression_ratio
);
criterion_main!(benches);
