use std::fmt;

#[derive(Debug)]
pub enum MonitorError {
    TracingError(tracing_subscriber::util::TryInitError),
    OtelError(String),
    InitError(String),
}

impl fmt::Display for MonitorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MonitorError::TracingError(e) => write!(f, "Tracing setup error: {}", e),
            MonitorError::OtelError(s) => write!(f, "OpenTelemetry error: {}", s),
            MonitorError::InitError(s) => write!(f, "Initialization error: {}", s),
        }
    }
}

impl std::error::Error for MonitorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MonitorError::TracingError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<tracing_subscriber::util::TryInitError> for MonitorError {
    fn from(e: tracing_subscriber::util::TryInitError) -> Self {
        MonitorError::TracingError(e)
    }
}
