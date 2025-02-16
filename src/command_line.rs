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
}

pub fn parse() -> Options {
    let opts = Options::parse();

    let debug_level = match opts.verbose {
        0 => tracing::Level::INFO,
        1 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    tracing_subscriber::fmt().with_max_level(debug_level).init();

    opts
}
