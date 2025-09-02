//! Generic logging to use throughtout twine

use eyre::Result;
use prometheus::Encoder;
use reth_tracing::tracing;
use reth_tracing::tracing_subscriber::EnvFilter;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::filter::Directive;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, Layer, Registry};

const DEFAULT_ENV_FILTER_DIRECTIVES: [&str; 4] = [
    "sqlx=off",
    "jsonrpsee-server=off",
    "hyper::proto::h1=off",
    "hyper_util=off",
];

fn build_filters() -> (EnvFilter, EnvFilter) {
    // stdout filter: default to "info", override with RUST_LOG
    let base_stdout = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let directives = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());

    let stdout_filter = DEFAULT_ENV_FILTER_DIRECTIVES
        .into_iter()
        .chain(directives.split(',').filter(|d| !d.is_empty()))
        .fold(base_stdout, |env_filter, directive| {
            let dir: Directive = directive.parse().unwrap();
            env_filter.add_directive(dir)
        });

    let base_file = EnvFilter::new("debug");
    let file_directives = "debug".to_owned();

    let file_filter = DEFAULT_ENV_FILTER_DIRECTIVES
        .into_iter()
        .chain(file_directives.split(',').filter(|d| !d.is_empty()))
        .fold(base_file, |env_filter, directive| {
            let dir: Directive = directive.parse().unwrap();
            env_filter.add_directive(dir)
        });

    (stdout_filter, file_filter)
}

/// Initialize logging for aggregator
pub fn init_with_config(metrics_addr: Option<String>, log_file_name: &str) -> Result<()> {
    let (stdout_filter, file_filter) = build_filters();

    // Create tracing subscriber
    let registry = Registry::default();

    // Add formatting layer for stdout with info and above logs
    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_file(true)
        .with_line_number(true)
        .with_filter(stdout_filter);
    let registry = registry.with(fmt_layer);

    // Add file logging layer for debug and above levels
    let file_appender = RollingFileAppender::new(Rotation::DAILY, "logs", log_file_name);
    let file_layer = fmt::layer()
        .with_writer(file_appender)
        .with_ansi(false)
        .with_filter(file_filter);
    let registry = registry.with(file_layer);

    // Set the global subscriber
    tracing::subscriber::set_global_default(registry)?;

    // Start metrics server if configured
    if let Some(addr) = metrics_addr {
        start_metrics_server(&addr)?;
    }

    Ok(())
}

/// Start HTTP server to expose Prometheus metrics
fn start_metrics_server(addr: &str) -> Result<()> {
    use std::net::SocketAddr;

    let addr: SocketAddr = addr.parse()?;

    std::thread::spawn(move || {
        let server = tiny_http::Server::http(addr).expect("Failed to start metrics server");

        for request in server.incoming_requests() {
            if request.url() == "/metrics" {
                let mut buffer = vec![];
                let encoder = prometheus::TextEncoder::new();
                let metric_families = prometheus::gather();

                if encoder.encode(&metric_families, &mut buffer).is_ok() {
                    let response = tiny_http::Response::from_data(buffer).with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], encoder.format_type())
                            .unwrap(),
                    );
                    let _ = request.respond(response);
                } else {
                    let response =
                        tiny_http::Response::from_string("Error encoding metrics".to_string())
                            .with_status_code(500);
                    let _ = request.respond(response);
                }
            } else {
                let response =
                    tiny_http::Response::from_string("Not found. Try /metrics".to_string())
                        .with_status_code(404);
                let _ = request.respond(response);
            }
        }
    });

    tracing::info!("Metrics server started at {}", addr);
    Ok(())
}
