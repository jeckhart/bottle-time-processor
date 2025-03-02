use crate::common::FakeInfluxDbClient;
use bottle_time_processor::{
    influxdb::InfluxDBWriter, mqtt_client::process_message, watchdog::Signal,
};
use influxdb2::models::WriteDataPoint;
use std::sync::Arc;
use tokio::sync::mpsc;

mod common; // Import the common helper module

#[tokio::test]
async fn test_process_message() {
    let fake_client = Arc::new(FakeInfluxDbClient::new());
    // Mock setup
    let mock_writer = InfluxDBWriter::with_client(fake_client.clone(), "test_bucket".to_string());

    // Sample power readings
    let message = r#"{"alias":"dev01","deviceId":"1234","power_total":7,"voltages_mv":[119355,119355,119276,119276,119171,119171,119171],"currents_ma":[23,23,23,23,23,23,22],"powers_mw":[754,757,757,739,739,770,770],"timestamps":[1740947155686,1740947158755,1740947161665,1740947164698,1740947167764,1740947170665,1740947172914],"num_readings":7}"#;
    let (reset_tx, mut reset_rx) = mpsc::channel(16);

    let result = process_message(message, &mock_writer, Some(reset_tx)).await;

    assert!(result.is_ok());

    assert_eq!(fake_client.data_points_written.lock().await.len(), 7);

    // Check the first data point
    let dp1 = &fake_client.data_points_written.lock().await[0];
    let mut v = Vec::new();
    dp1.write_data_point_to(&mut v).unwrap();
    let s = String::from_utf8(v).unwrap();

    assert_eq!(
        s,
        "power_reading,device_id=1234,device_name=dev01 current_ma=23,power_mw=754,voltage_mv=119355 1740947155686000000\n"
    );

    assert_eq!(reset_rx.recv().await.unwrap(), Signal::Reset);
}
