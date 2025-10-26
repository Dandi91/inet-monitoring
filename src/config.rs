use std::env;

pub struct Config {
    pub port: u16,
    pub targets: Vec<String>,
    pub delay: f32,
    pub timeout: f32,
    pub speedtest_interval: f32,
    pub speedtest_timeout: f32,
}

impl Config {
    pub fn load() -> Self {
        let port = env::var("PORT")
            .unwrap_or("9090".to_string())
            .parse::<u16>()
            .expect("invalid port");
        let targets = env::var("TARGETS")
            .unwrap_or("8.8.8.8".to_string())
            .split(',')
            .map(String::from)
            .collect();
        let delay = env::var("DELAY")
            .unwrap_or("5".to_string())
            .parse::<f32>()
            .expect("invalid delay");
        let timeout = env::var("TIMEOUT")
            .unwrap_or("5".to_string())
            .parse::<f32>()
            .expect("invalid timeout");
        let speedtest_interval = env::var("SPEEDTEST_INTERVAL")
            .unwrap_or("300".to_string())
            .parse::<f32>()
            .expect("invalid interval");
        let speedtest_timeout = env::var("SPEEDTEST_TIMEOUT")
            .unwrap_or("60".to_string())
            .parse::<f32>()
            .expect("invalid timeout");
        Config {
            port,
            targets,
            delay,
            timeout,
            speedtest_interval,
            speedtest_timeout,
        }
    }
}
