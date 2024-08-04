use std::collections::HashMap;

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{Sampler, TracerProvider},
};
use tracing::Subscriber;
use tracing_subscriber::{prelude::*, registry::LookupSpan, Layer};

pub fn setup() {
    global::set_text_map_propagator(TraceContextPropagator::new());

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .with(tracing_subscriber::filter::filter_fn(|meta| {
            meta.target() != "h2::proto::connection"
        }))
        .with(load_output_layer())
        .init();
}

pub fn shutdown() {
    global::shutdown_tracer_provider();
}

fn load_otlp_headers() -> HashMap<String, String> {
    let mut tracing_metadata = HashMap::new();

    #[cfg(debug_assertions)]
    tracing_metadata.insert("x-honeycomb-team".into(), "X6naTEMkzy10PMiuzJKifF".into());

    match std::env::var("OTEL_EXPORTER_OTLP_HEADERS").ok() {
        Some(headers) if !headers.is_empty() => {
            for header in headers.split_terminator(',') {
                if let Some((key, value)) = header.split_once('=') {
                    let key: &str = Box::leak(key.to_string().into_boxed_str());
                    let value = value.to_owned();
                    if let Ok(value) = value.parse() {
                        tracing_metadata.insert(key.into(), value);
                    } else {
                        eprintln!("Could not parse value for header {}.", key);
                    }
                }
            }
        }
        _ => {}
    }

    tracing_metadata
}

fn load_trace_sampler() -> Sampler {
    fn get_trace_ratio() -> f64 {
        std::env::var("OTEL_TRACES_SAMPLER_ARG")
            .ok()
            .and_then(|ratio| ratio.parse().ok())
            .unwrap_or(1.0)
    }

    std::env::var("OTEL_TRACES_SAMPLER")
        .map(|s| match s.as_str() {
            "always_on" => opentelemetry_sdk::trace::Sampler::AlwaysOn,
            "always_off" => opentelemetry_sdk::trace::Sampler::AlwaysOff,
            "traceidratio" => {
                opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(get_trace_ratio())
            }
            "parentbased_always_on" => opentelemetry_sdk::trace::Sampler::ParentBased(Box::new(
                opentelemetry_sdk::trace::Sampler::AlwaysOn,
            )),
            "parentbased_always_off" => opentelemetry_sdk::trace::Sampler::ParentBased(Box::new(
                opentelemetry_sdk::trace::Sampler::AlwaysOff,
            )),
            "parentbased_traceidratio" => opentelemetry_sdk::trace::Sampler::ParentBased(Box::new(
                opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(get_trace_ratio()),
            )),
            _ => opentelemetry_sdk::trace::Sampler::AlwaysOn,
        })
        .unwrap_or(opentelemetry_sdk::trace::Sampler::AlwaysOn)
}

fn load_output_layer<S>() -> Box<dyn Layer<S> + Send + Sync + 'static>
where
    S: Subscriber + Send + Sync,
    for<'a> S: LookupSpan<'a>,
{
    #[cfg(not(debug_assertions))]
    let tracing_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();

    #[cfg(debug_assertions)]
    let tracing_endpoint = Some("https://api.honeycomb.io/v1/traces".to_string());

    let client = reqwest::Client::new();

    if let Some(endpoint) = tracing_endpoint {
        let metadata = load_otlp_headers();
        let provider = TracerProvider::builder()
            .with_config(
                opentelemetry_sdk::trace::Config::default()
                    .with_resource(opentelemetry_sdk::Resource::new(vec![
                        opentelemetry::KeyValue::new("service.name", "github-backup"),
                        opentelemetry::KeyValue::new("service.version", version!("v")),
                        opentelemetry::KeyValue::new("host.os", std::env::consts::OS),
                        opentelemetry::KeyValue::new("host.architecture", std::env::consts::ARCH),
                    ]))
                    .with_sampler(load_trace_sampler()),
            )
            .with_batch_exporter(
                opentelemetry_otlp::new_exporter()
                    .http()
                    .with_protocol(
                        match std::env::var("OTEL_EXPORTER_OTLP_PROTOCOL").ok().as_deref() {
                            Some("http-binary") => opentelemetry_otlp::Protocol::HttpBinary,
                            Some("http-json") => opentelemetry_otlp::Protocol::HttpJson,
                            _ => opentelemetry_otlp::Protocol::HttpJson,
                        },
                    )
                    .with_endpoint(endpoint)
                    .with_headers(metadata)
                    .with_http_client(client)
                    .build_span_exporter()
                    .unwrap(),
                opentelemetry_sdk::runtime::Tokio,
            )
            .build();

        let tracer = provider.tracer("github-backup");

        tracing_opentelemetry::layer().with_tracer(tracer).boxed()
    } else {
        tracing_subscriber::fmt::layer().boxed()
    }
}
