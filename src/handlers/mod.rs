use aws_sdk_s3::error::ProvideErrorMetadata;
use axum::{
    extract::Path,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde::{Deserialize, Serialize};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn, Level};
use futures::stream::{StreamExt, TryStreamExt};

use crate::models::{AppState, CachedObject, BucketCredentials};

#[derive(Serialize)]
struct BucketStatus {
    name: String,
    bucket_name: String,
    endpoint: String,
    region: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    timestamp: u64,
    uptime_seconds: Option<u64>,
    buckets: Vec<BucketStatus>,
}

#[derive(Serialize)]
struct SuccessResponse {
    status: String,
    message: String,
    timestamp: u64,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
    key: String,
    timestamp: u64,
}

#[derive(Deserialize)]
pub struct RegisterBucketRequest {
    pub name: String,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub region: String,
}

/// Create a JSON error response
fn create_error_response(error_type: &str, message: &str, key: &str) -> Response {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let error_response = ErrorResponse {
        error: error_type.to_string(),
        message: message.to_string(),
        key: key.to_string(),
        timestamp,
    };

    match error_type {
        "bad_request" => (StatusCode::BAD_REQUEST, Json(error_response)).into_response(),
        "not_found" => (StatusCode::NOT_FOUND, Json(error_response)).into_response(),
        "unauthorized" => (StatusCode::UNAUTHORIZED, Json(error_response)).into_response(),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)).into_response(),
    }
}

/// Health check endpoint
pub async fn health_check(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    info!("Health check requested");

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let uptime_seconds = state.start_time.elapsed().as_secs();

    let collection = state.db.collection::<BucketCredentials>("buckets");
    let buckets = match collection.find(None, None).await {
        Ok(cursor) => {
            match cursor.try_collect::<Vec<_>>().await {
                Ok(all_buckets) => {
                    all_buckets.into_iter().map(|bucket| BucketStatus {
                        name: bucket.name,
                        bucket_name: bucket.bucket_name,
                        endpoint: bucket.endpoint,
                        region: bucket.region,
                    }).collect()
                }
                Err(e) => {
                    warn!("Error collecting buckets: {}", e);
                    Vec::new()
                }
            }
        }
        Err(e) => {
            warn!("Failed to fetch buckets: {}", e);
            Vec::new()
        }
    };

    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp,
        uptime_seconds: Some(uptime_seconds),
        buckets,
    };

    Json(response)
}

/// Reset cache endpoint
pub async fn reset_cache(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    info!("Cache reset requested");

    // Clear all cached objects
    state.cache.invalidate_all();
    state.cache.run_pending_tasks().await;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let response = SuccessResponse {
        status: "success".to_string(),
        message: "Cache has been reset".to_string(),
        timestamp,
    };

    Json(response)
}

/// Register a new bucket
pub async fn register_bucket(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RegisterBucketRequest>,
) -> Response {
    info!("Registering new bucket: {}", payload.name);

    // Check static API token
    let auth_header = headers.get("X-API-Token")
        .and_then(|h| h.to_str().ok());

    if auth_header != Some(&state.registration_token) {
        warn!("Unauthorized bucket registration attempt");
        return create_error_response("unauthorized", "Invalid or missing API token", "registration");
    }

    let bucket = BucketCredentials {
        id: None,
        name: payload.name,
        endpoint: payload.endpoint,
        access_key: payload.access_key,
        secret_key: payload.secret_key,
        bucket_name: payload.bucket_name,
        region: payload.region,
    };

    let collection = state.db.collection::<BucketCredentials>("buckets");
    
    match collection.insert_one(bucket, None).await {
        Ok(result) => {
            info!("Bucket registered successfully with id: {}", result.inserted_id);
            (StatusCode::CREATED, Json(serde_json::json!({
                "status": "success",
                "message": "Bucket registered successfully",
                "id": result.inserted_id
            }))).into_response()
        }
        Err(e) => {
            error!("Failed to store bucket in MongoDB: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to save bucket to database").into_response()
        }
    }
}

/// Get object from cache or S3
pub async fn get_object(
    Path((instance, key)): Path<(String, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Response {
    let request_start = Instant::now();
    let span = tracing::span!(Level::INFO, "get_object", instance = %instance, key = %key);
    let _enter = span.enter();

    // Validate and sanitize the key
    let key = match validate_and_sanitize_key(&key) {
        Ok(k) => k,
        Err(response) => return response,
    };

    // Use instance-prefixed key for cache to prevent collisions between buckets
    let cache_key = format!("{}:{}", instance, key);

    // Check cache first
    if let Some(cached) = state.cache.get(&cache_key).await {
        return handle_cache_hit(&key, cached, request_start);
    }

    // Cache miss - fetch from S3
    handle_cache_miss(&instance, &key, &state, request_start).await
}

/// Validate and sanitize the object key
fn validate_and_sanitize_key(key: &str) -> Result<String, Response> {
    // Remove leading slash
    let key = key.trim_start_matches('/');

    // Validate key is not empty
    if key.is_empty() {
        warn!("Empty key requested");
        return Err(create_error_response(
            "bad_request",
            "Key cannot be empty",
            key,
        ));
    }

    // Basic security: prevent directory traversal
    if key.contains("..") || key.starts_with('/') {
        warn!("Potentially malicious key requested: {}", key);
        return Err(create_error_response(
            "bad_request",
            "Invalid key format",
            key,
        ));
    }

    // Limit key length to prevent abuse
    if key.len() > 1024 {
        warn!("Key too long: {} characters", key.len());
        return Err(create_error_response("bad_request", "Key is too long", key));
    }

    Ok(key.to_string())
}

/// Handle cache hit
fn handle_cache_hit(key: &str, cached: CachedObject, request_start: Instant) -> Response {
    let duration = request_start.elapsed();
    info!(
        cache_hit = true,
        key = %key,
        content_type = ?cached.content_type,
        data_size = cached.data.len(),
        duration_ms = duration.as_millis()
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600".parse().unwrap(),
    );
    if let Some(ct) = &cached.content_type {
        headers.insert(header::CONTENT_TYPE, ct.parse().unwrap());
    } else {
        headers.insert(
            header::CONTENT_TYPE,
            "application/octet-stream".parse().unwrap(),
        );
    }

    (StatusCode::OK, headers, cached.data).into_response()
}

/// Helper to get or create an S3 client for an instance
async fn get_s3_client_for_instance(
    instance: &str,
    state: &AppState,
) -> Result<(aws_sdk_s3::Client, String), Response> {
    // Check if client is already in cache
    if let Some(cached_client) = state.s3_clients.get(instance).await {
        return Ok(cached_client);
    }

    // Not in cache, fetch credentials from MongoDB
    let collection = state.db.collection::<BucketCredentials>("buckets");
    let filter = mongodb::bson::doc! { "name": instance };
    
    let credentials = match collection.find_one(filter, None).await {
        Ok(Some(creds)) => creds,
        Ok(None) => {
            warn!("Instance not found: {}", instance);
            return Err(create_error_response("not_found", &format!("Instance '{}' not found", instance), instance));
        }
        Err(e) => {
            error!("Database error fetching instance {}: {}", instance, e);
            return Err(create_error_response("internal_error", "Database error", instance));
        }
    };

    // Create new S3 client
    let client = match crate::services::create_s3_client(&credentials).await {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to create S3 client for {}: {}", instance, e);
            return Err(create_error_response("internal_error", "Failed to initialize storage client", instance));
        }
    };

    let bucket_name = credentials.bucket_name.clone();
    let client_info = (client, bucket_name);
    
    // Cache the client
    state.s3_clients.insert(instance.to_string(), client_info.clone()).await;
    
    Ok(client_info)
}

/// Handle cache miss by fetching from S3
async fn handle_cache_miss(
    instance: &str,
    key: &str,
    state: &AppState,
    request_start: Instant,
) -> Response {
    let start_client = Instant::now();
    // Get client for this instance
    let (s3_client, bucket) = match get_s3_client_for_instance(instance, state).await {
        Ok(res) => res,
        Err(response) => return response,
    };
    let client_duration = start_client.elapsed();

    info!(
        cache_hit = false, 
        instance = %instance, 
        key = %key, 
        client_init_ms = client_duration.as_millis(),
        "Fetching from S3"
    );

    let start_s3 = Instant::now();
    let get_object_result = s3_client
        .get_object()
        .bucket(&bucket)
        .key(key)
        .send()
        .await;
    let s3_send_duration = start_s3.elapsed();

    match get_object_result {
        Ok(resp) => {
            info!(
                instance = %instance,
                key = %key,
                s3_send_ms = s3_send_duration.as_millis(),
                "S3 request successful, starting stream"
            );
            handle_s3_success(instance, key, resp, state, request_start).await
        },
        Err(e) => handle_s3_error(key, e, request_start),
    }
}

/// Handle successful S3 fetch
async fn handle_s3_success(
    instance: &str,
    key: &str,
    resp: aws_sdk_s3::operation::get_object::GetObjectOutput,
    state: &AppState,
    _request_start: Instant,
) -> Response {
    let content_type = resp.content_type().map(|s| s.to_string());
    let content_length = resp.content_length();
    
    // Convert ByteStream to a stream of Bytes for Axum
    let stream = resp.body;
    
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    let mut body_stream = stream;
    
    let cache_key = format!("{}:{}", instance, key);
    let state_clone = state.clone();
    let content_type_clone = content_type.clone();
    let key_clone = key.to_string();
    let instance_clone = instance.to_string();

    // Spawn a task to consume the S3 stream, send to Axum, and buffer for cache
    tokio::spawn(async move {
        let mut buffer = Vec::new();
        let mut first_chunk = true;
        let stream_start = Instant::now();
        
        while let Some(chunk) = body_stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if first_chunk {
                        info!(
                            instance = %instance_clone,
                            key = %key_clone,
                            ttfb_ms = stream_start.elapsed().as_millis(),
                            "First chunk received from S3"
                        );
                        first_chunk = false;
                    }
                    
                    let data = bytes.clone();
                    buffer.extend_from_slice(&data);
                    if tx.send(Ok::<bytes::Bytes, core::convert::Infallible>(data)).await.is_err() {
                        warn!("Client disconnected while streaming from S3");
                        return;
                    }
                }
                Err(e) => {
                    error!("Error streaming from S3: {}", e);
                    return;
                }
            }
        }
        
        let total_duration = stream_start.elapsed();
        info!(
            instance = %instance_clone,
            key = %key_clone,
            total_stream_ms = total_duration.as_millis(),
            size = buffer.len(),
            "Finished streaming from S3, caching object"
        );

        // Cache the object once fully received
        if !buffer.is_empty() {
            let cached_obj = CachedObject {
                data: buffer.into(),
                content_type: content_type_clone,
            };
            state_clone.cache.insert(cache_key, cached_obj).await;
        }
    });

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=3600".parse().unwrap(),
    );
    if let Some(ct) = &content_type {
        headers.insert(header::CONTENT_TYPE, ct.parse().unwrap());
    }
    if let Some(cl) = content_length {
        headers.insert(header::CONTENT_LENGTH, cl.into());
    }

    // Return the streaming response
    (StatusCode::OK, headers, axum::body::Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(rx))).into_response()
}

/// Handle S3 error
fn handle_s3_error(
    key: &str,
    error: aws_sdk_s3::error::SdkError<aws_sdk_s3::operation::get_object::GetObjectError>,
    request_start: Instant,
) -> Response {
    let duration = request_start.elapsed();

    // Check if it's a "Not Found" error
    if let aws_sdk_s3::error::SdkError::ServiceError(ref service_error) = error {
        if service_error.err().is_no_such_key() {
            warn!(
                key = %key,
                error = %error,
                duration_ms = duration.as_millis(),
                "Object not found in S3"
            );
            return create_error_response("not_found", "The requested file was not found", key);
        }
        // ponytail: private CDN — echo the S3 code so a misconfigured bucket is
        // diagnosable from the response instead of needing server logs.
        let code = service_error.err().code().unwrap_or("Unknown");
        error!(
            key = %key,
            s3_code = %code,
            error = ?error,
            duration_ms = duration.as_millis(),
            "S3 service error"
        );
        return create_error_response("internal_error", &format!("S3 error: {}", code), key);
    }

    // Other S3 errors - log with full details for debugging
    let error_msg = match &error {
        aws_sdk_s3::error::SdkError::ServiceError(se) => {
            format!("S3 service error: {:?}", se.err().code())
        }
        aws_sdk_s3::error::SdkError::DispatchFailure(_) => "Network/dispatch failure".to_string(),
        aws_sdk_s3::error::SdkError::TimeoutError(_) => "Request timeout".to_string(),
        aws_sdk_s3::error::SdkError::ResponseError(_) => {
            "Response error".to_string()
        }
        _ => "Unknown S3 error".to_string(),
    };

    error!(
        key = %key,
        error_type = %error_msg,
        full_error = ?error,
        duration_ms = duration.as_millis(),
        "S3 error occurred"
    );
    create_error_response(
        "internal_error",
        "An internal error occurred while retrieving the file",
        key,
    )
}
