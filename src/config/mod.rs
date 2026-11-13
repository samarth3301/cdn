use serde::Deserialize;
use std::env;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub cache: CacheConfig,
    #[allow(dead_code)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 9876,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub registration_token: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub uri: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfig {
    pub max_capacity: u64,
    pub time_to_live_seconds: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_capacity: 1000,
            time_to_live_seconds: 3600,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[allow(dead_code)]
    pub level: String,
    #[allow(dead_code)]
    pub format: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "default".to_string(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            server: ServerConfig::from_env()?,
            database: DatabaseConfig::from_env()?,
            auth: AuthConfig::from_env()?,
            cache: CacheConfig::from_env()?,
            logging: LoggingConfig::from_env()?,
        })
    }
}

impl ServerConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "9876".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidPort)?,
        })
    }
}

impl AuthConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            registration_token: env::var("REGISTRATION_TOKEN")
                .map_err(|_| ConfigError::MissingEnvVar("REGISTRATION_TOKEN"))?,
        })
    }
}

impl DatabaseConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            uri: env::var("MONGODB_URI").unwrap_or_else(|_| "mongodb://localhost:27017".to_string()),
            name: env::var("MONGODB_DATABASE").unwrap_or_else(|_| "cdn_storage".to_string()),
        })
    }
}

impl CacheConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            max_capacity: env::var("CACHE_MAX_CAPACITY")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidCacheCapacity)?,
            time_to_live_seconds: env::var("CACHE_TTL_SECONDS")
                .unwrap_or_else(|_| "3600".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidCacheTTL)?,
        })
    }
}

impl LoggingConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            level: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            format: env::var("LOG_FORMAT").unwrap_or_else(|_| "default".to_string()),
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(&'static str),
    #[error("Invalid port number")]
    InvalidPort,
    #[error("Invalid cache capacity")]
    InvalidCacheCapacity,
    #[error("Invalid cache TTL")]
    InvalidCacheTTL,
}
