// Unified multi-database connection pool manager using sqlx::AnyPool
use dashmap::DashMap; // Import DashMap for concurrent thread-safe pool caching
use serde::{Deserialize, Serialize}; // Import Serde traits for JSON struct serialization
use sqlx::any::AnyPoolOptions; // Import AnyPoolOptions from sqlx::any
use sqlx::AnyPool; // Import AnyPool from sqlx root crate
use std::sync::Arc; // Import Arc for atomic reference counting across threads
use std::time::{Duration, Instant}; // Import Duration and Instant for latency calculation

// Cloud IAM Authentication Protocol Parameters (GAP 14)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CloudIamConfig {
    pub provider: String, // "gcp_bigquery", "aws_redshift", "azure_ad"
    pub service_account_json_path: Option<String>,
    pub aws_role_arn: Option<String>,
    pub azure_client_id: Option<String>,
    pub azure_tenant_id: Option<String>,
}

// Structured connection parameters payload
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionDetails {
    pub db_type: String, // Database engine kind (postgres, mysql, sqlite, etc.)
    pub host: String, // Server host or IP address
    pub port: u16, // Server port number
    pub user: String, // Database user account
    pub password: Option<String>, // Password credential
    pub database: String, // Database or schema name / file path
    pub ssl_mode: Option<String>, // SSL / TLS mode (disable, require, verify-full)
    pub cloud_iam: Option<CloudIamConfig>, // Cloud IAM credentials (GAP 14)
    /// When true, the backend rejects write/DDL SQL and mutation IPC for this pool.
    #[serde(default)]
    pub is_read_only: bool,
}

// Result payload for connection testing diagnostics
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TestConnectionResult {
    pub success: bool, // True if TCP connection and authentication succeeded
    pub latency_ms: u64, // Roundtrip execution ping latency in milliseconds
    pub message: String, // Detailed message or exception diagnostics
}

/// Engines that sqlx AnyPool can actually open today (compiled drivers + compat layers).
pub fn is_supported_engine(db_type: &str) -> bool {
    matches!(
        db_type.to_lowercase().as_str(),
        "postgres"
            | "postgresql"
            | "mysql"
            | "mariadb"
            | "sqlite"
            | "cockroachdb"
            | "redshift"
            | "duckdb"
            | "turso"
            | "redis"
    )
}

/// Percent-encode credentials/path segments so passwords with @ : / etc. do not break URLs.
fn url_encode_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for b in value.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// Helper function to build sqlx connection URL string from structured parameters
pub fn build_connection_url(details: &ConnectionDetails) -> Result<String, String> {
    let db_kind = details.db_type.to_lowercase();

    // Reject engines that have UI entries but no real driver implementation.
    if matches!(
        db_kind.as_str(),
        "bigquery"
    ) {
        return Err(format!(
            "Database engine '{}' is not supported directly by the current driver matrix.",
            details.db_type
        ));
    }

    // DuckDB is handled by DuckDbManager (not sqlx AnyPool URL).
    if db_kind == "duckdb" {
        return Err(
            "DuckDB uses a dedicated connection path (file or :memory:), not a sqlx URL."
                .to_string(),
        );
    }

    if details.cloud_iam.is_some() {
        return Err(
            "Cloud IAM authentication is not implemented yet. Use username/password credentials."
                .to_string(),
        );
    }

    let user = url_encode_component(&details.user);
    let pass_str = details
        .password
        .as_deref()
        .map(url_encode_component)
        .unwrap_or_default();
    let database = url_encode_component(&details.database);

    match db_kind.as_str() {
        // SQLite embedded file/memory databases, plus DuckDB and Turso compatibility
        "sqlite" | "duckdb" | "turso" => {
            if details.database.starts_with("sqlite:") || details.database.starts_with("file:") {
                Ok(details.database.clone())
            } else if details.database.is_empty() || details.database == ":memory:" {
                Ok("sqlite::memory:".to_string())
            } else {
                Ok(format!("sqlite:{}?mode=rwc", details.database))
            }
        }
        "redis" => {
            if pass_str.is_empty() {
                Ok(format!("redis://{}:{}", details.host, details.port))
            } else {
                Ok(format!("redis://:{}@{}:{}", pass_str, details.host, details.port))
            }
        }
        "mysql" | "mariadb" => {
            if pass_str.is_empty() {
                Ok(format!(
                    "mysql://{}@{}:{}/{}",
                    user, details.host, details.port, database
                ))
            } else {
                Ok(format!(
                    "mysql://{}:{}@{}:{}/{}",
                    user, pass_str, details.host, details.port, database
                ))
            }
        }
        // Postgres wire-compatible engines (Cockroach / Redshift use the postgres protocol)
        "postgres" | "postgresql" | "cockroachdb" | "redshift" | _ => {
            let ssl = details.ssl_mode.as_deref().unwrap_or("require");
            let port = if details.port == 0 { 5432 } else { details.port };
            if pass_str.is_empty() {
                Ok(format!(
                    "postgres://{}@{}:{}/{}?sslmode={}",
                    user, details.host, port, database, ssl
                ))
            } else {
                Ok(format!(
                    "postgres://{}:{}@{}:{}/{}?sslmode={}",
                    user, pass_str, details.host, port, database, ssl
                ))
            }
        }
    }
}

use bb8::Pool;
use bb8_tiberius::ConnectionManager as MssqlConnectionManager;

/// Active pool plus the engine kind used to open it (needed for dialect-aware SQL).
#[derive(Clone)]
pub struct ManagedConnection {
    pub pool: AnyPool,
    pub pg_pool: Option<sqlx::PgPool>,
    pub mysql_pool: Option<sqlx::MySqlPool>,
    pub mssql_pool: Option<Pool<MssqlConnectionManager>>,
    pub mongo_client: Option<mongodb::Client>,
    pub redis_client: Option<redis::aio::MultiplexedConnection>,
    pub scylla_session: Option<std::sync::Arc<scylla::Session>>,
    pub clickhouse_client: Option<clickhouse::Client>,
    pub turso_config: Option<crate::db::turso_engine::TursoConfig>,
    pub db_type: String,
    pub connection_url: String,
    pub is_read_only: bool,
}

// Central connection manager struct holding active database pools
#[derive(Clone)]
pub struct ConnectionManager {
    pools: Arc<DashMap<String, ManagedConnection>>,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ConnectionManager {
    // Construct a new empty ConnectionManager instance
    pub fn new() -> Self {
        Self {
            pools: Arc::new(DashMap::new()),
        }
    }

    // Register sqlx default database drivers (Postgres, MySQL, SQLite)
    pub fn init_drivers() {
        sqlx::any::install_default_drivers();
    }

    /// True when identifiers should be backtick-quoted (MySQL/MariaDB).
    pub fn is_mysql_style(db_type: &str) -> bool {
        matches!(db_type.to_lowercase().as_str(), "mysql" | "mariadb")
    }

    // Test reachability and credentials for a connection config without storing pool
    pub async fn test_connection(details: &ConnectionDetails) -> TestConnectionResult {
        let start = Instant::now();
        let db_kind = details.db_type.to_lowercase();
        if db_kind == "turso" {
            return crate::db::turso_engine::test_turso_connection(details);
        }
        let url = match build_connection_url(details) {
            Ok(u) => u,
            Err(msg) => {
                return TestConnectionResult {
                    success: false,
                    latency_ms: start.elapsed().as_millis() as u64,
                    message: msg,
                };
            }
        };

        let pool_res = AnyPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(5))
            .connect(&url)
            .await;

        match pool_res {
            Ok(pool) => {
                // Execute lightweight ping query
                let ping_res = sqlx::query("SELECT 1;").execute(&pool).await;
                let elapsed = start.elapsed().as_millis() as u64;
                let _ = pool.close().await;

                match ping_res {
                    Ok(_) => TestConnectionResult {
                        success: true,
                        latency_ms: elapsed,
                        message: format!(
                            "Successfully connected to {} database ({}ms)",
                            details.db_type, elapsed
                        ),
                    },
                    Err(e) => TestConnectionResult {
                        success: false,
                        latency_ms: elapsed,
                        message: format!("Connection open succeeded but ping query failed: {}", e),
                    },
                }
            }
            Err(e) => TestConnectionResult {
                success: false,
                latency_ms: start.elapsed().as_millis() as u64,
                message: format!("Failed to connect to database: {}", e),
            },
        }
    }

    // Establish a connection pool for a connection string and store it
    pub async fn connect(&self, id: &str, url: &str, db_type: &str) -> Result<(), String> {
        self.connect_with_flags(id, url, db_type, false).await
    }

    pub async fn connect_with_flags(
        &self,
        id: &str,
        url: &str,
        db_type: &str,
        is_read_only: bool,
    ) -> Result<(), String> {
        // Pure in-memory SQLite URLs allocate a separate empty DB per connection.
        // Cap the pool at 1 (or use shared-cache URLs) so schema/data remain visible.
        let is_ephemeral_sqlite = {
            let lower = url.to_lowercase();
            lower.contains("sqlite::memory:")
                || lower.contains("mode=memory")
                || lower == "sqlite::memory:"
        };
        let max_conns = if is_ephemeral_sqlite { 1 } else { 10 };

        // Replace existing pool for this id if reconnecting
        if let Some((_, old)) = self.pools.remove(id) {
            old.pool.close().await;
        }

        let db_lower = db_type.to_lowercase();
        let mut pg_pool = None;
        let mut mysql_pool = None;
        let mut mssql_pool = None;
        let mut mongo_client = None;
        let mut redis_client = None;
        let mut scylla_session = None;
        let mut clickhouse_client = None;

        let pool = if matches!(db_lower.as_str(), "mssql" | "sqlserver") {
            let config = tiberius::Config::from_ado_string(url)
                .map_err(|e| format!("Invalid MSSQL ADO connection string: {}", e))?;
            let manager = MssqlConnectionManager::build(config)
                .map_err(|e| format!("Failed to build MSSQL manager: {}", e))?;
            let p = Pool::builder()
                .max_size(max_conns as u32)
                .build(manager)
                .await
                .map_err(|e| format!("Failed to create MSSQL pool: {}", e))?;
            mssql_pool = Some(p);
            // Dummy AnyPool to satisfy the struct's ABI (will not be used for routing)
            AnyPoolOptions::new().connect("sqlite::memory:").await.unwrap()
        } else if matches!(db_lower.as_str(), "mongodb") {
            // MongoDB init
            let client_options = mongodb::options::ClientOptions::parse(url)
                .await
                .map_err(|e| format!("Invalid MongoDB connection string: {}", e))?;
            let client = mongodb::Client::with_options(client_options)
                .map_err(|e| format!("Failed to initialize MongoDB client: {}", e))?;
            mongo_client = Some(client);
            AnyPoolOptions::new().connect("sqlite::memory:").await.unwrap()
        } else if matches!(db_lower.as_str(), "redis") {
            // Redis init
            let client = redis::Client::open(url)
                .map_err(|e| format!("Invalid Redis connection string: {}", e))?;
            let conn = client.get_multiplexed_tokio_connection().await
                .map_err(|e| format!("Failed to connect to Redis: {}", e))?;
            redis_client = Some(conn);
            AnyPoolOptions::new().connect("sqlite::memory:").await.unwrap()
        } else if matches!(db_lower.as_str(), "cassandra") {
            // Scylla/Cassandra init
            let session = scylla::SessionBuilder::new().known_node(url).build().await
                .map_err(|e| format!("Failed to connect to Cassandra/Scylla: {}", e))?;
            scylla_session = Some(std::sync::Arc::new(session));
            AnyPoolOptions::new().connect("sqlite::memory:").await.unwrap()
        } else if matches!(db_lower.as_str(), "clickhouse") {
            // Clickhouse init
            let client = clickhouse::Client::default().with_url(url);
            clickhouse_client = Some(client);
            AnyPoolOptions::new().connect("sqlite::memory:").await.unwrap()
        } else {
            AnyPoolOptions::new()
                .max_connections(max_conns)
                .acquire_timeout(Duration::from_secs(10))
                .connect(url)
                .await
                .map_err(|e| format!("Failed to connect to database: {}", e))?
        };

        match db_lower.as_str() {
            "postgres" | "postgresql" | "cockroachdb" | "redshift" => {
                use sqlx::postgres::PgPoolOptions;
                match PgPoolOptions::new()
                    .max_connections(max_conns)
                    .acquire_timeout(Duration::from_secs(10))
                    .connect(url)
                    .await
                {
                    Ok(pg) => pg_pool = Some(pg),
                    Err(e) => eprintln!("Warning: Failed to create native PgPool: {}", e),
                }
            }
            "mysql" | "mariadb" => {
                use sqlx::mysql::MySqlPoolOptions;
                if let Ok(my) = MySqlPoolOptions::new()
                    .max_connections(max_conns)
                    .acquire_timeout(Duration::from_secs(10))
                    .connect(url)
                    .await
                {
                    mysql_pool = Some(my);
                }
            }
            _ => {}
        }

        self.pools.insert(
            id.to_string(),
            ManagedConnection {
                pool,
                pg_pool,
                mysql_pool,
                mssql_pool,
                mongo_client,
                redis_client,
                scylla_session,
                clickhouse_client,
                turso_config: None,
                db_type: db_type.to_string(),
                connection_url: url.to_string(),
                is_read_only,
            },
        );
        Ok(())
    }

    // Connect using structured connection details
    pub async fn connect_with_details(
        &self,
        id: &str,
        details: &ConnectionDetails,
    ) -> Result<(), String> {
        let db_kind = details.db_type.to_lowercase();
        if db_kind == "turso" {
            let turso_cfg = crate::db::turso_engine::TursoConfig::from_details(details)?;
            let dummy_pool = AnyPoolOptions::new()
                .max_connections(1)
                .connect_lazy("sqlite::memory:")
                .unwrap();
            let managed = ManagedConnection {
                pool: dummy_pool,
                pg_pool: None,
                mysql_pool: None,
                mssql_pool: None,
                mongo_client: None,
                redis_client: None,
                scylla_session: None,
                clickhouse_client: None,
                turso_config: Some(turso_cfg),
                db_type: details.db_type.clone(),
                connection_url: details.host.clone(),
                is_read_only: details.is_read_only,
            };
            self.pools.insert(id.to_string(), managed);
            return Ok(());
        }
        let url = build_connection_url(details)?;
        self.connect_with_flags(id, &url, &details.db_type, details.is_read_only)
            .await
    }

    /// True when this connection was opened with the read-only flag.
    pub fn is_read_only(&self, id: &str) -> bool {
        self.pools
            .get(id)
            .map(|r| r.is_read_only)
            .unwrap_or(false)
    }

    /// Reject mutations when the pool is read-only.
    pub fn ensure_writes_allowed(&self, id: &str) -> Result<(), String> {
        if self.is_read_only(id) {
            return Err(
                "Connection is read-only. Write/DDL operations are blocked by the server."
                    .to_string(),
            );
        }
        Ok(())
    }

    // Retrieve an active connection pool reference by connection ID
    pub fn get_pool(&self, id: &str) -> Result<AnyPool, String> {
        self.pools
            .get(id)
            .map(|r| r.pool.clone())
            .ok_or_else(|| format!("Connection ID '{}' is not connected or pool expired", id))
    }

    pub fn get_managed_connection(&self, id: &str) -> Result<ManagedConnection, String> {
        self.pools
            .get(id)
            .map(|r| r.clone())
            .ok_or_else(|| format!("Connection ID '{}' is not connected or pool expired", id))
    }

    /// Engine kind recorded when the pool was opened (defaults to empty if unknown).
    pub fn get_db_type(&self, id: &str) -> Result<String, String> {
        self.pools
            .get(id)
            .map(|r| r.db_type.clone())
            .ok_or_else(|| format!("Connection ID '{}' is not connected or pool expired", id))
    }

    // Disconnect and remove a connection pool from cache
    pub async fn disconnect(&self, id: &str) -> Result<(), String> {
        if let Some((_, managed)) = self.pools.remove(id) {
            managed.pool.close().await;
        }
        Ok(())
    }

    /// IDs of currently open pools (multi-connection workspace).
    pub fn list_connected_ids(&self) -> Vec<String> {
        self.pools.iter().map(|e| e.key().clone()).collect()
    }

    pub fn is_connected(&self, id: &str) -> bool {
        self.pools.contains_key(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_manager_instantiation() {
        ConnectionManager::init_drivers();
        let manager = ConnectionManager::new();
        assert!(manager.get_pool("invalid_id").is_err());
    }

    #[test]
    fn test_url_builder() {
        let details = ConnectionDetails {
            db_type: "postgres".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            user: "postgres".to_string(),
            password: Some("secret".to_string()),
            database: "testdb".to_string(),
            ssl_mode: Some("disable".to_string()),
            cloud_iam: None,
            is_read_only: false,
        };
        let url = build_connection_url(&details).unwrap();
        assert_eq!(
            url,
            "postgres://postgres:secret@localhost:5432/testdb?sslmode=disable"
        );
    }

    #[test]
    fn test_url_builder_encodes_special_password_chars() {
        let details = ConnectionDetails {
            db_type: "postgres".to_string(),
            host: "localhost".to_string(),
            port: 5432,
            user: "user@corp".to_string(),
            password: Some("p@ss:w/rd".to_string()),
            database: "testdb".to_string(),
            ssl_mode: Some("disable".to_string()),
            cloud_iam: None,
            is_read_only: false,
        };
        let url = build_connection_url(&details).unwrap();
        assert_eq!(
            url,
            "postgres://user%40corp:p%40ss%3Aw%2Frd@localhost:5432/testdb?sslmode=disable"
        );
    }

    #[test]
    fn test_unsupported_engine_rejected() {
        let details = ConnectionDetails {
            db_type: "bigquery".to_string(),
            host: "localhost".to_string(),
            port: 27017,
            user: "admin".to_string(),
            password: None,
            database: "test".to_string(),
            ssl_mode: None,
            cloud_iam: None,
            is_read_only: false,
        };
        let err = build_connection_url(&details).unwrap_err();
        assert!(err.contains("not supported"));
    }

    #[test]
    fn test_cockroachdb_and_redshift_supported() {
        assert!(is_supported_engine("cockroachdb"));
        assert!(is_supported_engine("redshift"));

        let crdb_details = ConnectionDetails {
            db_type: "cockroachdb".to_string(),
            host: "crdb.cluster.cloud".to_string(),
            port: 26257,
            user: "dev".to_string(),
            password: Some("pass".to_string()),
            database: "bank".to_string(),
            ssl_mode: Some("verify-full".to_string()),
            cloud_iam: None,
            is_read_only: false,
        };
        let url = build_connection_url(&crdb_details).unwrap();
        assert_eq!(
            url,
            "postgres://dev:pass@crdb.cluster.cloud:26257/bank?sslmode=verify-full"
        );

        let redshift_details = ConnectionDetails {
            db_type: "redshift".to_string(),
            host: "rs.dw.amazonaws.com".to_string(),
            port: 5439,
            user: "awsuser".to_string(),
            password: Some("Secret123".to_string()),
            database: "analytics".to_string(),
            ssl_mode: Some("require".to_string()),
            cloud_iam: None,
            is_read_only: false,
        };
        let rs_url = build_connection_url(&redshift_details).unwrap();
        assert_eq!(
            rs_url,
            "postgres://awsuser:Secret123@rs.dw.amazonaws.com:5439/analytics?sslmode=require"
        );
    }


    #[test]
    fn test_duckdb_and_turso_supported() {
        assert!(is_supported_engine("duckdb"));
        assert!(is_supported_engine("turso"));

        let duck_details = ConnectionDetails {
            db_type: "duckdb".to_string(),
            host: "".to_string(),
            port: 0,
            user: "".to_string(),
            password: None,
            database: "analytics.duckdb".to_string(),
            ssl_mode: None,
            cloud_iam: None,
            is_read_only: false,
        };
        let duck_err = build_connection_url(&duck_details).unwrap_err();
        assert!(duck_err.contains("dedicated connection path"));

        let turso_details = ConnectionDetails {
            db_type: "turso".to_string(),
            host: "".to_string(),
            port: 0,
            user: "".to_string(),
            password: None,
            database: ":memory:".to_string(),
            ssl_mode: None,
            cloud_iam: None,
            is_read_only: false,
        };
        let turso_url = build_connection_url(&turso_details).unwrap();
        assert_eq!(turso_url, "sqlite::memory:");
    }
}


