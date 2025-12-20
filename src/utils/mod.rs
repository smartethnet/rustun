use std::net::Ipv6Addr;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

pub mod device;
pub mod sys_route;

pub fn init_tracing() -> Result<(), Box<dyn std::error::Error>> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::FmtSubscriber::builder()
            .with_env_filter(
                EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env_lossy(),
            )
            .with_line_number(true)
            .with_file(true)
            .finish(),
    )?;
    Ok(())
}

/// Get public IPv6 address from external API
pub fn get_ipv6() -> Option<String> {
    let apis = [
        "https://api64.ipify.org",
        "https://ifconfig.co",
        "https://ipv6.icanhazip.com",
    ];

    for api in &apis {
        if let Ok(ipv6) = fetch_ipv6_from_url(api) {
            tracing::info!("Retrieved public IPv6 from {}: {}", api, ipv6);
            return Some(ipv6);
        }
    }

    None
}

fn fetch_ipv6_from_url(url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response = ureq::get(url)
        .timeout(std::time::Duration::from_secs(5))
        .call()?
        .into_string()?;

    let ipv6_str = response.trim();

    // Validate it's a proper IPv6 address
    ipv6_str.parse::<Ipv6Addr>()?;

    Ok(ipv6_str.to_string())
}
