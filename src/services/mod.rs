use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::{config::Region, Client};
use moka::future::Cache;
use mongodb::{options::ClientOptions, Client as MongoClient, Database};
use std::time::{Duration, Instant};
use tracing::{info, error};
use futures::StreamExt;

use crate::config::{CacheConfig, DatabaseConfig, AuthConfig};
use crate::models::{AppState, CachedObject, BucketCredentials};

/// Create S3 client from bucket credentials
pub async fn create_s3_client(
    credentials: &BucketCredentials,
) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
    info!(
        "Creating S3 client with endpoint: {}, bucket: {}, region: {}",
        credentials.endpoint, credentials.bucket_name, credentials.region
    );

    let aws_config = aws_config::defaults(BehaviorVersion::v2025_08_07())
        .region(Region::new(credentials.region.clone()))
        .endpoint_url(&credentials.endpoint)
        .credentials_provider(Credentials::new(
            &credentials.access_key,
            &credentials.secret_key,
            None,
            None,
            "r2",
        ))
        .load()
        .await;

    info!("Cloudflare R2 authentication successful - S3 client configured");
    Ok(Client::new(&aws_config))
}

/// Create cache from configuration
pub fn create_cache(config: &CacheConfig) -> Cache<String, CachedObject> {
    let cache = Cache::builder()
        .max_capacity(config.max_capacity)
        .time_to_live(Duration::from_secs(config.time_to_live_seconds))
        .build();

    info!(
        "Cache configured: max_capacity={}, ttl={}s",
        config.max_capacity, config.time_to_live_seconds
    );

    cache
}

/// Create MongoDB connection
pub async fn create_db_connection(
    config: &DatabaseConfig,
) -> Result<Database, Box<dyn std::error::Error + Send + Sync>> {
    info!("Connecting to MongoDB at: {}", config.uri);
    let client_options = ClientOptions::parse(&config.uri).await?;
    let client = MongoClient::with_options(client_options)?;
    let db = client.database(&config.name);
    
    // Ping the database to verify connection
    db.run_command(mongodb::bson::doc! {"ping": 1}, None).await?;
    info!("Successfully connected to MongoDB: {}", config.name);
    
    Ok(db)
}

/// Create application state
pub async fn create_app_state(
    cache_config: &CacheConfig,
    db_config: &DatabaseConfig,
    auth_config: &AuthConfig,
) -> Result<AppState, Box<dyn std::error::Error + Send + Sync>> {
    let cache = create_cache(cache_config);
    let db = create_db_connection(db_config).await?;

    // Create a cache for S3 clients
    let s3_clients = Cache::builder()
        .max_capacity(100) // Support up to 100 different bucket instances
        .time_to_live(Duration::from_secs(3600)) // TTL of 1 hour for clients
        .build();

    // Fetch all registered buckets from the database
    let collection = db.collection::<BucketCredentials>("buckets");
    let mut cursor = collection.find(None, None).await?;
    
    let mut count = 0;
    while let Some(result) = cursor.next().await {
        match result {
            Ok(creds) => {
                match create_s3_client(&creds).await {
                    Ok(client) => {
                        info!("[+] Connected to - {}", creds.name);
                        s3_clients.insert(creds.name.clone(), (client, creds.bucket_name.clone())).await;
                        count += 1;
                    }
                    Err(e) => {
                        error!("[-] Failed to connect to - {}: {}", creds.name, e);
                    }
                }
            }
            Err(e) => {
                error!("Error reading bucket from database: {}", e);
            }
        }
    }

    if count == 0 {
        info!("No buckets found in database. Waiting for registration...");
    } else {
        info!("Successfully pre-connected to {} bucket(s)", count);
    }

    Ok(AppState {
        s3_clients,
        cache,
        db,
        registration_token: auth_config.registration_token.clone(),
        start_time: Instant::now(),
    })
}
