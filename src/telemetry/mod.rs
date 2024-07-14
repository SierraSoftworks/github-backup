use opentelemetry::global;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{propagation::TraceContextPropagator, trace::Sampler};
use tracing::Subscriber;
use tracing_subscriber::{prelude::*, registry::LookupSpan, Layer};

pub fn setup() {
    global::set_text_map_propagator(TraceContextPropagator::new());

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::LevelFilter::DEBUG)
        .with(tracing_subscriber::filter::dynamic_filter_fn(
            |_metadata, ctx| {
                !ctx.lookup_current()
                    // Exclude the rustls session "Connection" events which don't have a parent span
                    .map(|s| s.parent().is_none() && s.name() == "Connection")
                    .unwrap_or_default()
            },
        ))
        .with(load_output_layer())
        .init();
}

fn load_otlp_headers() -> tonic::metadata::MetadataMap {
    let mut tracing_metadata = tonic::metadata::MetadataMap::new();

    #[cfg(debug_assertions)]
    tracing_metadata.insert(
        "x-honeycomb-team",
        "X6naTEMkzy10PMiuzJKifF".parse().unwrap(),
    );

    match std::env::var("OTEL_EXPORTER_OTLP_HEADERS").ok() {
        Some(headers) if !headers.is_empty() => {
            for header in headers.split_terminator(',') {
                if let Some((key, value)) = header.split_once('=') {
                    let key: &str = Box::leak(key.to_string().into_boxed_str());
                    let value = value.to_owned();
                    if let Ok(value) = value.parse() {
                        tracing_metadata.insert(key, value);
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
    let tracing_endpoint = Some("https://api.honeycomb.io:443".to_string());

    if let Some(endpoint) = tracing_endpoint {
        let metadata = load_otlp_headers();
        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(endpoint)
                    .with_metadata(metadata),
            )
            .with_trace_config(
                opentelemetry_sdk::trace::config()
                    .with_resource(opentelemetry_sdk::Resource::new(vec![
                        opentelemetry::KeyValue::new("service.name", "github-backup"),
                        opentelemetry::KeyValue::new("service.version", version!("v")),
                        opentelemetry::KeyValue::new("host.os", std::env::consts::OS),
                        opentelemetry::KeyValue::new("host.architecture", std::env::consts::ARCH),
                    ]))
                    .with_sampler(load_trace_sampler()),
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .unwrap();

        tracing_opentelemetry::layer().with_tracer(tracer).boxed()
    } else {
        tracing_subscriber::fmt::layer().boxed()
    }
}
