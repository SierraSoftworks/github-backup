mod traced_stream;

pub use traced_stream::*;

use tracing_batteries::*;

pub fn setup() -> Session {
    Session::new("github-backup", version!())
        .with_battery(OpenTelemetry::new("").with_protocol(OpenTelemetryProtocol::HttpJson))
}
