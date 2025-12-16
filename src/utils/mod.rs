use tracing::level_filters::LevelFilter;
use tracing_subscriber::EnvFilter;

pub mod sys_route;
pub mod device;


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
