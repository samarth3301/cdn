# S3 CDN Network

A high-performance, distributed CDN (Content Delivery Network) service designed to bridge multiple S3-compatible buckets. This service provides a unified caching layer, reducing latency and egress costs by serving frequently accessed objects from an in-memory cache.

## 🚀 Features

- **Multi-Tenant S3 Support**: Connect and serve content from multiple independent S3-compatible buckets (AWS S3, MinIO, DigitalOcean Spaces, etc.).
- **High-Performance Caching**: Powered by the `moka` cache for efficient, asynchronous in-memory object storage.
- **Dynamic Bucket Registration**: Register new storage instances on-the-fly via a secure API.
- **Auto-Sanitization**: Built-in security to prevent directory traversal and malicious key requests.
- **Lightweight & Fast**: Built with Rust and Axum for maximum throughput and minimal resource footprint.

## 🛠️ Architecture

The service acts as a transparent proxy between your users and your S3 buckets:
1. **Request**: User requests `http://cdn.yourdomain.com/{instance-name}/{object-key}`.
2. **Cache Check**: The CDN checks if the object is in the local memory cache.
3. **S3 Fetch**: On a cache miss, it identifies the correct bucket credentials from MongoDB and fetches the object from S3.
4. **Response**: The object is cached and served to the user with appropriate Cache-Control headers.

## 📋 Prerequisites

- Rust (latest stable)
- MongoDB (for storing bucket credentials)
- S3-compatible storage buckets

## ⚙️ Configuration

Create a `.env` file in the root directory:

```env
# Server
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# MongoDB
MONGODB_URI=mongodb://localhost:27017
DATABASE_NAME=cdn_network

# Security
REGISTRATION_TOKEN=your-secure-registration-token

# Cache Settings
CACHE_MAX_CAPACITY=10000
CACHE_TIME_TO_IDLE_SECONDS=3600
```

## 🚀 Getting Started

1. **Clone and Build**:
   ```bash
   cargo build --release
   ```

2. **Run the Server**:
   ```bash
   cargo run --release
   ```

## 📡 API Reference

### 1. Register a Bucket
Register a new S3 instance that the CDN should serve.

```bash
curl -X POST http://localhost:8080/buckets/register \
     -H "Content-Type: application/json" \
     -H "X-API-Token: your-secure-registration-token" \
     -d '{
       "name": "assets-bucket",
       "endpoint": "https://s3.amazonaws.com",
       "access_key": "YOUR_ACCESS_KEY",
       "secret_key": "YOUR_SECRET_KEY",
       "bucket_name": "production-assets",
       "region": "us-east-1"
     }'
```

### 2. Access Content
Access any file using the instance name as the first path segment.

```bash
# Pattern: /{instance-name}/{path/to/file}
curl http://localhost:8080/assets-bucket/images/banner.jpg
```

### 3. Management APIs

- **Health Check**: `GET /health`
- **Reset Cache**: `POST /reset-cache`

## 🛡️ Security

- **Registration Token**: The `/buckets/register` endpoint is protected by a static API token.
- **Key Validation**: All object keys are sanitized to prevent directory traversal attacks.
- **Concurrency Limit**: Built-in protection against request flooding using Tower middleware.

## 📄 License

This project is licensed under the MIT License.
