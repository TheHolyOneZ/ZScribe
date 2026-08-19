use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init() {
    let filter = EnvFilter::try_from_env("ZSCRIBE_LOG")
        .unwrap_or_else(|_| EnvFilter::new("info,zscribe=debug"));

    let registry = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true).compact());

    match zscribe_store::Paths::from_env() {
        Ok(paths) => {
            let appender = tracing_appender::rolling::daily(paths.logs_dir(), "zscribe.log");
            registry
                .with(fmt::layer().with_ansi(false).with_writer(appender))
                .init();
        }
        Err(err) => {
            registry.init();
            tracing::warn!(%err, "could not resolve the log directory; logging to stdout only");
        }
    }
}
