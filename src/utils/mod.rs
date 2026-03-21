use std::{
    net::Ipv6Addr,
    time::{Duration, Instant},
};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

pub mod device;
pub mod sys_route;

#[derive(Debug, Clone, PartialEq)]
pub struct StunAddr {
    pub ip: String,
    pub port: u16,
}
impl std::fmt::Display for StunAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.ip, self.port)
    }
}

pub fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    // On Windows, disable ANSI colors to avoid garbage characters in console
    // On Unix systems, keep ANSI colors for better readability
    #[cfg(target_os = "windows")]
    let use_ansi = false;

    #[cfg(not(target_os = "windows"))]
    let use_ansi = true;

    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env_lossy(),
            )
            .with_ansi(use_ansi) // Disable ANSI colors on Windows
            .with_line_number(true)
            .with_file(true)
            .finish(),
    )?;
    Ok(())
}

/// Get public IPv6 address from external API
pub async fn get_ipv6() -> Option<Ipv6Addr> {
    let apis = [
        "https://api64.ipify.org",
        "https://ifconfig.co",
        "https://ipv6.icanhazip.com",
    ];

    for api in &apis {
        if let Ok(ipv6) = fetch_ipv6_from_url(api).await {
            return Some(ipv6);
        }
    }

    None
}

async fn fetch_ipv6_from_url(url: &str) -> anyhow::Result<Ipv6Addr> {
    use tokio::time::timeout_at;
    let deadline = Instant::now() + Duration::from_secs(5);
    let get = timeout_at(deadline.into(), reqwest::get(url)).await??;
    let response = timeout_at(deadline.into(), get.text()).await??;

    let ipv6_str = response.trim();
    Ok(ipv6_str.parse::<Ipv6Addr>()?)
}
