# Telemetry
GitHub Backup is designed to run as an unattended backup system, requiring minimal
human involvement to maintain and operate. To provide visibility into the status of
your backups, it includes support for [OpenTelemetry](https://opentelemetry.io)
tracing which can be used to monitor the health of your backups and ensure that
they are running as expected.

By default GitHub Backup will export its telemetry data to the console in a human
readable format, however you can configure it to export this data to any OpenTelemetry
compatible endpoint by configuring the following environment variables.

## Configuration

```bash
# Required: configure the endpoint to which telemetry data should be sent
OTEL_EXPORTER_OTLP_ENDPOINT=https://your-otel-collector:4317

# Optional: configure the protocol to use when sending telemetry data (`grpc`, `http-json`, `http-binary`)
OTEL_EXPORTER_OTLP_PROTOCOL=grpc

# Optional: configure headers to be sent to the OTLP endpoint
OTEL_EXPORTER_OTLP_HEADERS=X-API-KEY=your-api-key

# Optional: configure the sampler to use for tracing data (`always_on`, `always_off`, `traceidratio`)
OTEL_TRACES_SAMPLER=always_on

# Optional: configure the argument to pass to the sampler, used to configure the sampling ratio (a number between 0 and 1)
OTEL_TRACES_SAMPLER_ARG=1.0
```

## Examples

### Honeycomb
If you are using [Honeycomb.io](https://honeycomb.io) for your telemetry data, you can
configure GitHub Backup to send its telemetry data to your Honeycomb account by setting
the following environment variables.

```bash
OTEL_EXPORTER_OTLP_ENDPOINT="https://api.honeycomb.io:443"
OTEL_EXPORTER_OTLP_HEADERS="x-honeycomb-team=<honeycomb_api_key>"
```

### Grafana Cloud
If you are using [Grafana Cloud](https://grafana.com/cloud) for your telemetry data, you can
configure GitHub Backup to send its telemetry data to your Grafana Cloud account by setting
the following environment variables.

::: tip
This configuration will match the configuration provided to you by Grafana Cloud when
[configuring OTLP ingestion](https://grafana.com/docs/grafana-cloud/send-data/otlp/send-data-otlp/).
:::

```bash
OTEL_EXPORTER_OTLP_ENDPOINT="https://otlp-gateway-prod-eu-west-0.granfa.net/otlp"
OTEL_EXPORTER_OTLP_HEADERS="Authorization=Basic <base64_encoded_api_key>"
OTEL_EXPORTER_OTLP_PROTOCOL="http-binary"
```
