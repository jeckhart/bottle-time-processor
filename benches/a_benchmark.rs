use bottle_time_processor::{
    influxdb::InfluxDBWriter, models::KasaPowerMessage,
    test_utils::message_bench_utils::FakeInfluxDbClient,
};
use criterion::{Criterion, criterion_group, criterion_main};
use std::sync::Arc;

use tokio::runtime::Runtime;

pub fn add_benchmark(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut rvg = bottle_time_processor::test_utils::Rvg::deterministic();

    // Create FakeInfluxDbClient
    let fake_influxdb_client = FakeInfluxDbClient::new(false);
    let influx_writer =
        InfluxDBWriter::with_client(Arc::new(fake_influxdb_client), "test_bucket".to_string());

    c.bench_function("message_into_reading", move |b| {
        let message = Arc::new(KasaPowerMessage {
            alias: "Test Device".to_string(),
            device_id: "test-device".to_string(),
            power_total: 0,
            voltages_mv: vec![
                rvg.sample(&(11000..13000i32)),
                rvg.sample(&(11000..13000i32)),
                rvg.sample(&(11000..13000i32)),
            ],
            currents_ma: vec![
                rvg.sample(&(23..1300i32)),
                rvg.sample(&(23..1300i32)),
                rvg.sample(&(23..1300i32)),
            ],
            powers_mw: vec![
                rvg.sample(&(800..1300i32)),
                rvg.sample(&(800..1300i32)),
                rvg.sample(&(800..1300i32)),
            ],
            timestamps: vec![
                rvg.sample(&(1614556800..1614556860i64)),
                rvg.sample(&(1614556861..1614556920i64)),
                rvg.sample(&(1614556921..1614556980i64)),
            ],
            num_readings: 3,
        });
        b.iter(move || {
            let _readings = <KasaPowerMessage as Clone>::clone(&message)
                .clone()
                .into_readings();
        })
    });

    let reading = KasaPowerMessage {
        alias: "Test Device".to_string(),
        device_id: "test-device".to_string(),
        power_total: 0,
        voltages_mv: vec![12000],
        currents_ma: vec![100],
        powers_mw: vec![1200],
        timestamps: vec![1614556800],
        num_readings: 1,
    }
    .into_readings()
    .pop()
    .unwrap();

    c.bench_function("write_power_reading", |b| {
        b.to_async(&rt).iter(|| async {
            influx_writer.write_power_reading(&reading).await.unwrap();
        })
    });
}

criterion_group!(benches, add_benchmark);
criterion_main!(benches);
