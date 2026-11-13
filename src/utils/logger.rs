use std::env;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter, Registry,
};

/// Configuration for the logger
#[derive(Debug, Clone)]
pub struct LoggerConfig {
    /// Log level filter (e.g., "info", "debug", "warn", "error")
    pub level: String,
    /// Whether to use JSON format for logs
    pub json_format: bool,
    /// Whether to include file and line number information
    pub with_file_info: bool,
    /// Whether to include thread IDs
    pub with_thread_ids: bool,
    /// Whether to include thread names
    pub with_thread_names: bool,
    /// Whether to include span events
    pub with_span_events: bool,
    /// Custom target filters
    pub target_filters: Vec<String>,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            json_format: false,
            with_file_info: true,
            with_thread_ids: false,
            with_thread_names: false,
            with_span_events: false,
            target_filters: vec![
                "s3_cdn_network=info".to_string(),
                "tower_http=debug".to_string(),
                "axum=debug".to_string(),
            ],
        }
    }
}

impl LoggerConfig {
    /// Create a new logger configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        // Check for RUST_LOG environment variable
        if let Ok(rust_log) = env::var("RUST_LOG") {
            config.target_filters = rust_log.split(',').map(|s| s.trim().to_string()).collect();
        }

        // Check for LOG_FORMAT
        if let Ok(format) = env::var("LOG_FORMAT") {
            config.json_format = format.to_lowercase() == "json";
        }

        // Check for LOG_LEVEL
        if let Ok(level) = env::var("LOG_LEVEL") {
            config.level = level.to_lowercase();
        }

        // Check for LOG_THREAD_INFO
        if let Ok(thread_info) = env::var("LOG_THREAD_INFO") {
            let thread_info = thread_info.to_lowercase();
            config.with_thread_ids = thread_info.contains("id");
            config.with_thread_names = thread_info.contains("name");
        }

        // Check for LOG_SPAN_EVENTS
        if let Ok(span_events) = env::var("LOG_SPAN_EVENTS") {
            config.with_span_events = span_events.to_lowercase() == "true";
        }

        // Check for LOG_FILE_INFO
        if let Ok(file_info) = env::var("LOG_FILE_INFO") {
            config.with_file_info = file_info.to_lowercase() != "false";
        }

        config
    }

    /// Build the filter string from configuration
    fn build_filter(&self) -> String {
        if self.target_filters.is_empty() {
            format!(
                "{}= {}",
                env!("CARGO_PKG_NAME").replace("-", "_"),
                self.level
            )
        } else {
            self.target_filters.join(",")
        }
    }
}

/// Initialize the logger with the given configuration
pub fn init_logger() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    init_logger_with_config(LoggerConfig::from_env())
}

/// Initialize the logger with custom configuration
pub fn init_logger_with_config(
    config: LoggerConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = EnvFilter::try_new(config.build_filter())
        .map_err(|e| format!("Failed to create log filter: {}", e))?;

    let registry = Registry::default().with(filter);

    if config.json_format {
        // JSON format for production/structured logging
        let json_layer = fmt::layer()
            .json()
            .with_target(true)
            .with_file(config.with_file_info)
            .with_line_number(config.with_file_info)
            .with_thread_ids(config.with_thread_ids)
            .with_thread_names(config.with_thread_names);

        let json_layer = if config.with_span_events {
            json_layer.with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        } else {
            json_layer
        };

        registry.with(json_layer).try_init()?;
    } else {
        // Human-readable format for development
        let fmt_layer = fmt::layer()
            .with_target(false)
            .with_file(config.with_file_info)
            .with_line_number(config.with_file_info)
            .with_thread_ids(config.with_thread_ids)
            .with_thread_names(config.with_thread_names)
            .compact();

        let fmt_layer = if config.with_span_events {
            fmt_layer.with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        } else {
            fmt_layer
        };

        registry.with(fmt_layer).try_init()?;
    }

    Ok(())
}

/// Get the current logger configuration for inspection
#[allow(dead_code)]
pub fn get_current_config() -> LoggerConfig {
    LoggerConfig::from_env()
}
/// Test function to verify logging is working
#[cfg(test)]
#[allow(dead_code)]
pub fn test_logging() {
    use tracing::{debug, error, info, warn};

    info!("Logger test: Info level message");
    warn!("Logger test: Warning level message");
    error!("Logger test: Error level message");
    debug!("Logger test: Debug level message");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LoggerConfig::default();
        assert_eq!(config.level, "info");
        assert!(!config.json_format);
        assert!(config.with_file_info);
        assert!(!config.target_filters.is_empty());
    }

    #[test]
    fn test_config_from_env() {
        // Test with default environment (no env vars set)
        let config = LoggerConfig::from_env();
        assert_eq!(config.level, "info"); // Should use default since no LOG_LEVEL set
        assert!(!config.json_format); // Should be false since no LOG_FORMAT=json
        assert!(!config.with_thread_ids); // Should be false since no LOG_THREAD_INFO
        assert!(!config.with_thread_names); // Should be false since no LOG_THREAD_INFO
        assert!(!config.with_span_events); // Should be false since no LOG_SPAN_EVENTS
        assert!(config.with_file_info); // Should be true since no LOG_FILE_INFO=false
    }

    #[test]
    fn test_build_filter() {
        let config = LoggerConfig::default();
        let filter = config.build_filter();
        assert!(filter.contains("cdn_techsolace=info"));
    }
}
