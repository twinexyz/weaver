use std::env;

use anyhow::Result;
use tracing_subscriber::filter::Directive;
use tracing_subscriber::EnvFilter;

const DEFAULT_ENV_FILTER_DIRECTIVES: [&str; 3] =
    ["sqlx=off", "jsonrpsee-server=off", "hyper::proto::h1=off"];

fn build_env_filter(default_directive: Option<Directive>, directives: &str) -> Result<EnvFilter> {
    let env_filter = if let Some(default_directive) = default_directive {
        EnvFilter::builder()
            .with_default_directive(default_directive)
            .from_env_lossy()
    } else {
        EnvFilter::builder().from_env_lossy()
    };

    DEFAULT_ENV_FILTER_DIRECTIVES
        .into_iter()
        .chain(directives.split(',').filter(|d| !d.is_empty()))
        .try_fold(env_filter, |env_filter, directive| {
            Ok(env_filter.add_directive(directive.parse()?))
        })
}

pub fn init_logger(log_level: &str) {
    let log_level = env::var("RUST_LOG").unwrap_or(log_level.to_string());
    let build_filter = build_env_filter(None, &log_level).expect("failed to build log filter");

    tracing_subscriber::fmt()
        .with_env_filter(build_filter)
        .with_line_number(true)
        .with_file(true)
        .with_target(false)
        .init();
}
