use aws_sdk_s3::Client;
use bytes::Bytes;
use moka::future::Cache;
use mongodb::Database;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone)]
pub struct CachedObject {
    pub data: Bytes,
    pub content_type: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BucketCredentials {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<mongodb::bson::oid::ObjectId>,
    pub name: String,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub region: String,
}

#[derive(Clone)]
pub struct AppState {
    pub s3_clients: Cache<String, (Client, String)>, // (Client, bucket_name)
    pub cache: Cache<String, CachedObject>,
    pub db: Database,
    pub registration_token: String,
    pub start_time: Instant,
}
