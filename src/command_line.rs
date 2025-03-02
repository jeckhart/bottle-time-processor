use clap::Parser;

#[derive(Parser)]
#[clap(
    version,
    about = "Processor to read events from MQTT and identify bottle-time events"
)]
pub struct Options {
    /// Verbosity level (-v = debug, -vv = trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// MQTT broker hostname or IP address
    #[arg(long, env = "MQTT_BROKER", default_value = "localhost")]
    pub broker: String,

    /// MQTT broker port number
    #[arg(long, env = "MQTT_PORT", default_value_t = 1883)]
    pub port: u16,

    /// MQTT authentication username
    #[arg(long, env = "MQTT_USERNAME", default_value = "username")]
    pub username: String,

    /// MQTT authentication password
    #[arg(
        long,
        env = "MQTT_PASSWORD",
        default_value = "password",
        hide_env_values = true
    )]
    pub password: String,

    /// MQTT topic to subscribe for events
    #[arg(long, env = "MQTT_TOPIC", default_value = "username/feeds/topic1")]
    pub topic: String,

    /// InfluxDB URL
    #[arg(long, env = "INFLUXDB_URL", default_value = "http://localhost:8086")]
    pub influxdb_url: String,

    /// InfluxDB token
    #[arg(long, env = "INFLUXDB_TOKEN")]
    pub influxdb_token: String,

    /// InfluxDB organization
    #[arg(long, env = "INFLUXDB_ORG")]
    pub influxdb_org: String,

    /// InfluxDB bucket
    #[arg(long, env = "INFLUXDB_BUCKET")]
    pub influxdb_bucket: String,

    /// Enabled a watchdog timer to restart the application if it stops receiving events
    #[cfg(feature = "systemd")]
    #[arg(long, env = "WATCHDOG_TIMER", default_value = "systemd")]
    pub watchdog_time: String,

    #[cfg(not(feature = "systemd"))]
    #[arg(long, env = "WATCHDOG_TIMER", default_value = "900")]
    pub watchdog_time: String,
}

pub fn parse<I>(args: Option<I>) -> Options
where
    I: IntoIterator,
    I::Item: Into<String>,
{
    let args: Vec<String> = args.map_or_else(
        || {
            std::env::args_os()
                .map(|x| x.into_string().unwrap())
                .collect()
        },
        |x| x.into_iter().map(|x| x.into()).collect(),
    );

    let opts = Options::try_parse_from(args).unwrap();

    let debug_level = match opts.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    if let Err(e) = tracing_subscriber::fmt()
        .with_max_level(debug_level)
        .try_init()
    {
        eprintln!("Failed to set global subscriber: {}", e);
    }

    opts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Ignoring this test because it will fail if any of the environment variables are set
    fn test_defaults() {
        let opts = Options::try_parse_from(
            vec![
                "bottle-time-processor",
                "--influxdb-token",
                "token",
                "--influxdb-org",
                "org",
                "--influxdb-bucket",
                "bucket",
            ]
            .iter(),
        )
        .unwrap();
        assert_eq!(opts.broker, "localhost");
        assert_eq!(opts.port, 1883);
        assert_eq!(opts.username, "username");
        assert_eq!(opts.password, "password");
        assert_eq!(opts.topic, "username/feeds/topic1");
        assert_eq!(opts.influxdb_url, "http://localhost:8086");
    }

    #[test]
    fn test_parse_verbose() {
        let opts = Options::parse_from(&[
            "test",
            "-vv",
            "--influxdb-token",
            "token",
            "--influxdb-org",
            "org",
            "--influxdb-bucket",
            "bucket",
        ]);
        assert_eq!(opts.verbose, 2);
    }

    #[test]
    fn test_parse() {
        let args = vec![
            "bottle-time-processor",
            "--influxdb-token",
            "token",
            "--influxdb-org",
            "org",
            "--influxdb-bucket",
            "bucket",
        ];
        let opts = parse(Some(args.iter().map(|x| x.to_string())));

        assert_eq!(opts.influxdb_token, "token");
        assert_eq!(opts.influxdb_org, "org");
        assert_eq!(opts.influxdb_bucket, "bucket");
    }

    #[test]
    fn test_verbose() {
        let args = vec![
            "bottle-time-processor",
            "-v",
            "--influxdb-token",
            "token",
            "--influxdb-org",
            "org",
            "--influxdb-bucket",
            "bucket",
        ];
        let opts = parse(Some(args.iter().map(|x| x.to_string())));

        assert_eq!(opts.verbose, 1);
    }

    #[test]
    fn test_very_verbose() {
        let args = vec![
            "bottle-time-processor",
            "-vv",
            "--influxdb-token",
            "token",
            "--influxdb-org",
            "org",
            "--influxdb-bucket",
            "bucket",
        ];
        let opts = parse(Some(args.iter().map(|x| x.to_string())));

        assert_eq!(opts.verbose, 2);
    }

    #[ignore]
    #[test]
    /// Not yet working, this test fails because the "args" that clap picks up are the args
    /// from the test command. I don't know how to fix this yet.
    fn test_all_args() {
        unsafe {
            std::env::set_var("INFLUX_TOKEN", "token");
            std::env::set_var("INFLUX_ORG", "org");
            std::env::set_var("INFLUX_BUCKET", "bucket");
        }
        let opts = parse::<Vec<String>>(None);

        assert_eq!(opts.influxdb_token, "token");
        assert_eq!(opts.influxdb_org, "org");
        assert_eq!(opts.influxdb_bucket, "bucket");
    }
}
