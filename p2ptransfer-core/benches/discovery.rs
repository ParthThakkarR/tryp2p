use p2ptransfer_core::p2p::peer::PeerInfo;
use p2ptransfer_core::p2p::protocol::Beacon;
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use std::net::SocketAddr;

fn bench_beacon_serialize(c: &mut Criterion) {
    let beacon = Beacon {
        device_name: "test-device".into(),
        p2ptransfer_version: "0.1.0".into(),
        tcp_port: 9877,
    };

    let mut group = c.benchmark_group("discovery/serialize");

    group.bench_function("serialize_beacon", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&beacon).unwrap();
            black_box(json)
        })
    });

    let json = serde_json::to_string(&beacon).unwrap();
    group.bench_function("deserialize_beacon", |b| {
        b.iter(|| {
            let parsed: Beacon = serde_json::from_str(&json).unwrap();
            black_box(parsed)
        })
    });

    group.finish();
}

fn bench_peer_info(c: &mut Criterion) {
    let addr: SocketAddr = "192.168.1.100:9877".parse().unwrap();

    let mut group = c.benchmark_group("discovery/peer_info");

    group.bench_function("new", |b| {
        b.iter(|| {
            let info = PeerInfo::new("test-device".into(), 9877, addr);
            black_box(info)
        })
    });

    let mut info = PeerInfo::new("test-device".into(), 9877, addr);
    group.bench_function("is_stale", |b| {
        b.iter(|| {
            let stale = info.is_stale(std::time::Duration::from_secs(10));
            black_box(stale)
        })
    });

    group.bench_function("touch", |b| {
        b.iter(|| {
            info.touch();
            black_box(())
        })
    });

    group.finish();
}

fn bench_hashmap_peer_ops(c: &mut Criterion) {
    use std::collections::HashMap;

    let mut group = c.benchmark_group("discovery/hashmap");

    group.bench_function("insert_1000", |b| {
        b.iter(|| {
            let mut map: HashMap<String, PeerInfo> = HashMap::new();
            for i in 0..1000 {
                let addr: SocketAddr = format!("192.168.1.{}:9877", i % 255).parse().unwrap();
                map.insert(
                    format!("peer-{i}"),
                    PeerInfo::new(format!("device-{i}"), 9877, addr),
                );
            }
            black_box(map)
        })
    });

    let mut map: HashMap<String, PeerInfo> = HashMap::new();
    for i in 0..1000 {
        let addr: SocketAddr = format!("192.168.1.{}:9877", i % 255).parse().unwrap();
        map.insert(
            format!("peer-{i}"),
            PeerInfo::new(format!("device-{i}"), 9877, addr),
        );
    }

    group.bench_function("get_known", |b| {
        b.iter(|| {
            let found = map.get("peer-42");
            black_box(found)
        })
    });

    group.bench_function("get_unknown", |b| {
        b.iter(|| {
            let found = map.get("nonexistent");
            black_box(found)
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_beacon_serialize,
    bench_peer_info,
    bench_hashmap_peer_ops
);
criterion_main!(benches);
