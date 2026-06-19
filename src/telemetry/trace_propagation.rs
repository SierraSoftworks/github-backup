use tracing_batteries::prelude::*;

/// An [`OpenTelemetryPropagationInjector`] which writes the fields emitted by
/// the global text map propagator (such as the W3C `traceparent` header) into a
/// [`reqwest`] header map, silently skipping anything that is not a valid HTTP
/// header.
struct HeaderInjector<'a>(&'a mut reqwest::header::HeaderMap);

impl OpenTelemetryPropagationInjector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(name) = reqwest::header::HeaderName::from_bytes(key.as_bytes())
            && let Ok(value) = reqwest::header::HeaderValue::from_str(&value)
        {
            self.0.insert(name, value);
        }
    }
}

/// Builds the set of trace propagation headers for the provided OpenTelemetry
/// [`context`](opentelemetry::Context) using the globally configured text map
/// propagator.
///
/// When the context does not carry a valid span (for example because telemetry
/// export is disabled) no headers are produced, leaving the request untouched.
fn trace_context_headers(context: &opentelemetry::Context) -> reqwest::header::HeaderMap {
    let mut headers = reqwest::header::HeaderMap::new();
    get_text_map_propagator(|propagator| {
        propagator.inject_context(context, &mut HeaderInjector(&mut headers));
    });
    headers
}

/// Extension trait for [`reqwest::RequestBuilder`] which attaches the current
/// span's trace context to an outgoing request, allowing downstream services to
/// correlate their telemetry with the backup run that issued the request.
pub trait TracePropagationExt {
    /// Injects the current span's trace context (for example the W3C
    /// `traceparent` header) into the request so that it can be tied back to the
    /// originating trace when investigating cross-service failures.
    fn with_trace_context(self) -> Self;
}

impl TracePropagationExt for reqwest::RequestBuilder {
    fn with_trace_context(self) -> Self {
        let headers = trace_context_headers(&Span::current().context());
        self.headers(headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::{SpanContext, SpanId, TraceFlags, TraceId, TraceState};

    #[test]
    fn injects_traceparent_for_valid_context() {
        set_text_map_propagator(TraceContextPropagator::new());

        let span_context = SpanContext::new(
            TraceId::from_hex("0af7651916cd43dd8448eb211c80319c").unwrap(),
            SpanId::from_hex("b7ad6b7169203331").unwrap(),
            TraceFlags::SAMPLED,
            true,
            TraceState::default(),
        );
        let context = opentelemetry::Context::new().with_remote_span_context(span_context);

        let headers = trace_context_headers(&context);

        assert_eq!(
            headers.get("traceparent").and_then(|v| v.to_str().ok()),
            Some("00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01"),
        );
    }

    #[test]
    fn no_headers_for_empty_context() {
        set_text_map_propagator(TraceContextPropagator::new());

        let headers = trace_context_headers(&opentelemetry::Context::new());

        assert!(headers.get("traceparent").is_none());
    }
}
