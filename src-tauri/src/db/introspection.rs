// Database schema introspection engine for table, column, and Primary Key analysis
use serde::{Deserialize, Serialize}; // Import Serde traits for JSON serialization and deserialization
use sqlx::Any; // Import Any database driver marker type from sqlx
use sqlx::AnyPool; // Import AnyPool from sqlx for dynamic queries
use sqlx::Row; // Import Row trait for accessing dynamic database columns
use crate::db::pool::ManagedConnection;

// Data structure representing metadata about a database table or view
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Derive common standard traits
pub struct TableInfo {
    /// Bare object name (without schema)
    pub name: String,
    /// Schema / namespace (e.g. public, main, dbo)
    #[serde(default)]
    pub schema: String,
    /// BASE TABLE | VIEW | SYSTEM VIEW | …
    pub table_type: String,
    /// Schema-qualified name used for queries (schema.name or name)
    #[serde(default)]
    pub qualified_name: String,
}

impl TableInfo {
    pub fn new(schema: &str, name: &str, table_type: &str) -> Self {
        let schema = schema.to_string();
        let name = name.to_string();
        let qualified_name = if schema.is_empty() || schema == "main" {
            name.clone()
        } else {
            format!("{}.{}", schema, name)
        };
        Self {
            name,
            schema,
            table_type: table_type.to_string(),
            qualified_name,
        }
    }
}

/// Split `schema.table` or bare `table` into (schema, table).
/// Default schema is engine-dependent and applied by callers when None.
pub fn split_schema_table(name: &str) -> (Option<String>, String) {
    if let Some((schema, table)) = name.rsplit_once('.') {
        if !schema.is_empty() && !table.is_empty() {
            return (Some(schema.to_string()), table.to_string());
        }
    }
    (None, name.to_string())
}

// Data structure representing column properties
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Derive standard traits for JSON encoding
pub struct ColumnInfo { // Struct definition for column information
    pub name: String, // Column identifier name
    pub data_type: String, // SQL data type name (e.g. VARCHAR, INT, TIMESTAMP)
    pub is_nullable: bool, // Flag indicating whether null values are allowed
    pub is_primary_key: bool, // Flag indicating whether column is part of primary key
    #[serde(default)]
    pub is_foreign_key: bool, // True when this column references another table
    #[serde(default)]
    pub fk_table: Option<String>, // Referenced table name
    #[serde(default)]
    pub fk_column: Option<String>, // Referenced column name
} // End of ColumnInfo struct definition

/// Foreign-key edge used by ERD / relation navigation
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ForeignKeyInfo {
    pub column_name: String,
    pub referenced_table: String,
    pub referenced_column: String,
    pub constraint_name: String,
}

// Data structure holding primary key analysis result for table editing safety
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Derive traits for JSON serialization
pub struct PkAnalysis { // Struct definition for primary key analysis
    pub has_single_pk: bool, // True if table has exactly one primary key column
    pub pk_column_name: Option<String>, // Name of primary key column(s)
    pub pk_columns: Vec<String>, // List of primary key column names
    pub is_read_only: bool, // Read-only flag set if table cannot be safely updated inline
    pub read_only_reason: Option<String>, // User-facing explanation for read-only status
} // End of PkAnalysis struct definition


/// Reject table identifiers that could be used for SQL injection in catalog queries.
fn validate_identifier(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Table name cannot be empty".to_string());
    }
    if name.len() > 128 {
        return Err("Table name is too long".to_string());
    }
    // Allow schema-qualified names like public.users and quoted-safe alphanumerics/underscore
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
    {
        return Err(format!(
            "Invalid table identifier '{}': only alphanumeric, underscore, and dot are allowed",
            name
        ));
    }
    Ok(())
}

// Fetch list of tables and views from all user-visible schemas
pub async fn fetch_tables(pool: &AnyPool, db_kind: &str) -> Result<Vec<TableInfo>, String> {
    let mut tables = Vec::new();

    match db_kind.to_lowercase().as_str() {
        "postgres" | "postgresql" | "cockroachdb" | "redshift" => {
            // All non-system schemas: tables + views (DataGrip/TablePlus object browser)
            let sql = "SELECT table_schema, table_name, table_type
                FROM information_schema.tables
                WHERE table_schema NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
                  AND table_schema NOT LIKE 'pg_temp_%'
                  AND table_schema NOT LIKE 'pg_toast_temp_%'
                ORDER BY table_schema, table_type, table_name;";
            let rows = sqlx::query::<Any>(sql)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            for row in rows {
                let schema: String = row.get(0);
                let name: String = row.get(1);
                let table_type: String = row.get(2);
                tables.push(TableInfo::new(&schema, &name, &table_type));
            }
        }
        "mysql" | "mariadb" => {
            // Current database only (MySQL "schema" == database). Include tables + views.
            let sql = "SELECT table_schema, table_name, table_type
                FROM information_schema.tables
                WHERE table_schema = DATABASE()
                ORDER BY table_type, table_name;";
            let rows = sqlx::query::<Any>(sql)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            for row in rows {
                let schema: String = row.get(0);
                let name: String = row.get(1);
                let table_type: String = row.get(2);
                // Use bare name for default DB to keep SQL simple; still store schema
                let mut info = TableInfo::new(&schema, &name, &table_type);
                info.qualified_name = name.clone();
                tables.push(info);
            }
        }
        _ => {
            let sql = "SELECT name, type FROM sqlite_master
                WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%'
                ORDER BY type, name;";
            let rows = sqlx::query::<Any>(sql)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            for row in rows {
                let name: String = row.get(0);
                let ty: String = row.get(1);
                let table_type = if ty.eq_ignore_ascii_case("view") {
                    "VIEW"
                } else {
                    "BASE TABLE"
                };
                tables.push(TableInfo::new("main", &name, table_type));
            }
        }
    }

    Ok(tables)
}

/// Managed routing fetch_tables leveraging native drivers (pg_pool, mysql_pool, etc.).
pub async fn fetch_tables_managed(conn: &ManagedConnection) -> Result<Vec<TableInfo>, String> {
    let db_kind = conn.db_type.to_lowercase();
    match db_kind.as_str() {
        "postgres" | "postgresql" | "cockroachdb" | "redshift" => {
            if let Some(ref pg_pool) = conn.pg_pool {
                let sql = "SELECT table_schema, table_name, table_type
                    FROM information_schema.tables
                    WHERE table_schema NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
                      AND table_schema NOT LIKE 'pg_temp_%'
                      AND table_schema NOT LIKE 'pg_toast_temp_%'
                    ORDER BY table_schema, table_type, table_name;";
                let rows = sqlx::query(sql)
                    .fetch_all(pg_pool)
                    .await
                    .map_err(|e| format!("Postgres table introspection error: {}", e))?;
                let mut tables = Vec::new();
                for row in rows {
                    let schema: String = row.get(0);
                    let name: String = row.get(1);
                    let table_type: String = row.get(2);
                    tables.push(TableInfo::new(&schema, &name, &table_type));
                }
                return Ok(tables);
            }
        }
        "mysql" | "mariadb" => {
            if let Some(ref mysql_pool) = conn.mysql_pool {
                let sql = "SELECT table_schema, table_name, table_type
                    FROM information_schema.tables
                    WHERE table_schema = DATABASE()
                    ORDER BY table_type, table_name;";
                let rows = sqlx::query(sql)
                    .fetch_all(mysql_pool)
                    .await
                    .map_err(|e| format!("MySQL table introspection error: {}", e))?;
                let mut tables = Vec::new();
                for row in rows {
                    let schema: String = row.get(0);
                    let name: String = row.get(1);
                    let table_type: String = row.get(2);
                    let mut info = TableInfo::new(&schema, &name, &table_type);
                    info.qualified_name = name.clone();
                    tables.push(info);
                }
                return Ok(tables);
            }
        }
        "mongodb" => {
            if let Some(ref client) = conn.mongo_client {
                let db_name = if conn.connection_url.contains('/') {
                    conn.connection_url.rsplit('/').next().unwrap_or("test").split('?').next().unwrap_or("test")
                } else {
                    "test"
                };
                if let Ok(names) = client.database(db_name).list_collection_names().await {
                    let mut tables = Vec::new();
                    for name in names {
                        tables.push(TableInfo::new("mongodb", &name, "COLLECTION"));
                    }
                    return Ok(tables);
                }
            }
        }
        "redis" => {
            return Ok(vec![
                TableInfo::new("redis", "keys", "KEYSPACE"),
            ]);
        }
        "turso" => {
            if let Some(ref cfg) = conn.turso_config {
                return crate::db::turso_engine::fetch_turso_tables(cfg);
            }
        }
        _ => {}
    }
    fetch_tables(&conn.pool, &conn.db_type).await
}

/// Managed routing fetch_columns leveraging native drivers.
pub async fn fetch_columns_managed(
    conn: &ManagedConnection,
    table_name: &str,
) -> Result<Vec<ColumnInfo>, String> {
    validate_identifier(table_name)?;
    let (schema_opt, bare_table) = split_schema_table(table_name);
    let db_kind = conn.db_type.to_lowercase();

    if db_kind == "turso" {
        if let Some(ref cfg) = conn.turso_config {
            return crate::db::turso_engine::fetch_turso_columns(cfg, table_name);
        }
    }

    match db_kind.as_str() {
        "postgres" | "postgresql" | "cockroachdb" | "redshift" => {
            if let Some(ref pg_pool) = conn.pg_pool {
                let schema = schema_opt.unwrap_or_else(|| "public".to_string());
                let sql = "SELECT c.column_name, c.data_type, c.is_nullable,
                            CASE WHEN kcu.column_name IS NOT NULL THEN true ELSE false END as is_pk
                     FROM information_schema.columns c
                     LEFT JOIN information_schema.table_constraints tc
                       ON c.table_name = tc.table_name AND c.table_schema = tc.table_schema AND tc.constraint_type = 'PRIMARY KEY'
                     LEFT JOIN information_schema.key_column_usage kcu
                       ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema AND c.column_name = kcu.column_name
                     WHERE c.table_name = $1 AND c.table_schema = $2
                     ORDER BY c.ordinal_position";
                let rows = sqlx::query(sql)
                    .bind(&bare_table)
                    .bind(&schema)
                    .fetch_all(pg_pool)
                    .await
                    .map_err(|e| format!("Postgres column introspection error: {}", e))?;
                let mut columns = Vec::new();
                for row in rows {
                    let name: String = row.get(0);
                    let data_type: String = row.get(1);
                    let is_nullable_str: String = row.get(2);
                    let is_pk: bool = row.get(3);
                    columns.push(ColumnInfo {
                        name,
                        data_type,
                        is_nullable: is_nullable_str == "YES",
                        is_primary_key: is_pk,
                        is_foreign_key: false,
                        fk_table: None,
                        fk_column: None,
                    });
                }
                return Ok(columns);
            }
        }
        "mysql" | "mariadb" => {
            if let Some(ref mysql_pool) = conn.mysql_pool {
                let sql = if let Some(ref _schema) = schema_opt {
                    "SELECT column_name, data_type, is_nullable, column_key
                     FROM information_schema.columns
                     WHERE table_name = ? AND table_schema = ?
                     ORDER BY ordinal_position"
                } else {
                    "SELECT column_name, data_type, is_nullable, column_key
                     FROM information_schema.columns
                     WHERE table_name = ? AND table_schema = DATABASE()
                     ORDER BY ordinal_position"
                };
                let rows = if let Some(ref schema) = schema_opt {
                    sqlx::query(sql)
                        .bind(&bare_table)
                        .bind(schema)
                        .fetch_all(mysql_pool)
                        .await
                        .map_err(|e| format!("MySQL column introspection error: {}", e))?
                } else {
                    sqlx::query(sql)
                        .bind(&bare_table)
                        .fetch_all(mysql_pool)
                        .await
                        .map_err(|e| format!("MySQL column introspection error: {}", e))?
                };
                let mut columns = Vec::new();
                for row in rows {
                    let name: String = row.get(0);
                    let data_type: String = row.get(1);
                    let is_nullable_str: String = row.get(2);
                    let column_key: String = row.get(3);
                    columns.push(ColumnInfo {
                        name,
                        data_type,
                        is_nullable: is_nullable_str == "YES",
                        is_primary_key: column_key == "PRI",
                        is_foreign_key: false,
                        fk_table: None,
                        fk_column: None,
                    });
                }
                return Ok(columns);
            }
        }
        _ => {}
    }
    fetch_columns(&conn.pool, &conn.db_type, table_name).await
}

// Fetch column details for a table. Accepts bare name or schema.qualified name.
pub async fn fetch_columns(
    pool: &AnyPool,
    db_kind: &str,
    table_name: &str,
) -> Result<Vec<ColumnInfo>, String> {
    validate_identifier(table_name)?;
    let (schema_opt, bare_table) = split_schema_table(table_name);
    let mut columns = Vec::new();

    match db_kind.to_lowercase().as_str() {
        "postgres" | "postgresql" | "cockroachdb" | "redshift" => {
            let schema = schema_opt.unwrap_or_else(|| "public".to_string());
            let sql = "SELECT c.column_name, c.data_type, c.is_nullable,
                        CASE WHEN kcu.column_name IS NOT NULL THEN true ELSE false END as is_pk
                 FROM information_schema.columns c
                 LEFT JOIN information_schema.table_constraints tc
                   ON c.table_name = tc.table_name AND c.table_schema = tc.table_schema AND tc.constraint_type = 'PRIMARY KEY'
                 LEFT JOIN information_schema.key_column_usage kcu
                   ON tc.constraint_name = kcu.constraint_name AND tc.table_schema = kcu.table_schema AND c.column_name = kcu.column_name
                 WHERE c.table_name = $1 AND c.table_schema = $2
                 ORDER BY c.ordinal_position";
            let rows = sqlx::query::<Any>(sql)
                .bind(&bare_table)
                .bind(&schema)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            for row in rows {
                let name: String = row.get(0);
                let data_type: String = row.get(1);
                let is_nullable_str: String = row.get(2);
                let is_pk: bool = row.get(3);
                columns.push(ColumnInfo {
                    name,
                    data_type,
                    is_nullable: is_nullable_str == "YES",
                    is_primary_key: is_pk,
                    is_foreign_key: false,
                    fk_table: None,
                    fk_column: None,
                });
            }
        }
        "mysql" | "mariadb" => {
            let sql = if let Some(ref schema) = schema_opt {
                // Explicit schema (rare for single-DB connections)
                let _ = schema;
                "SELECT column_name, data_type, is_nullable, column_key
                 FROM information_schema.columns
                 WHERE table_name = ? AND table_schema = ?
                 ORDER BY ordinal_position"
            } else {
                "SELECT column_name, data_type, is_nullable, column_key
                 FROM information_schema.columns
                 WHERE table_name = ? AND table_schema = DATABASE()
                 ORDER BY ordinal_position"
            };
            let rows = if let Some(ref schema) = schema_opt {
                sqlx::query::<Any>(sql)
                    .bind(&bare_table)
                    .bind(schema)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| e.to_string())?
            } else {
                sqlx::query::<Any>(sql)
                    .bind(&bare_table)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| e.to_string())?
            };
            for row in rows {
                let name: String = row.get(0);
                let data_type: String = row.get(1);
                let is_nullable_str: String = row.get(2);
                let column_key: String = row.get(3);
                columns.push(ColumnInfo {
                    name,
                    data_type,
                    is_nullable: is_nullable_str == "YES",
                    is_primary_key: column_key == "PRI",
                    is_foreign_key: false,
                    fk_table: None,
                    fk_column: None,
                });
            }
        }
        _ => {
            // SQLite: strip schema prefix for PRAGMA
            let safe = bare_table.replace('"', "\"\"");
            let sql = format!("PRAGMA table_info(\"{}\");", safe);
            let rows = sqlx::query::<Any>(&sql)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            for row in rows {
                let name: String = row.get(1);
                let data_type: String = row.get(2);
                let notnull: i64 = row.get(3);
                let pk: i64 = row.get(5);
                columns.push(ColumnInfo {
                    name,
                    data_type,
                    is_nullable: notnull == 0,
                    is_primary_key: pk > 0,
                    is_foreign_key: false,
                    fk_table: None,
                    fk_column: None,
                });
            }
        }
    }

    // Enrich with foreign-key metadata (pass original name so schema is preserved)
    if let Ok(fks) = fetch_foreign_keys(pool, db_kind, table_name).await {
        for fk in fks {
            if let Some(col) = columns.iter_mut().find(|c| c.name == fk.column_name) {
                col.is_foreign_key = true;
                col.fk_table = Some(fk.referenced_table);
                col.fk_column = Some(fk.referenced_column);
            }
        }
    }

    Ok(columns)
}

/// Fetch foreign-key relationships for a table (Postgres / MySQL / SQLite).
/// Accepts bare or schema.qualified table names.
pub async fn fetch_foreign_keys(
    pool: &AnyPool,
    db_kind: &str,
    table_name: &str,
) -> Result<Vec<ForeignKeyInfo>, String> {
    validate_identifier(table_name)?;
    let (schema_opt, bare_table) = split_schema_table(table_name);
    let mut fks = Vec::new();

    match db_kind.to_lowercase().as_str() {
        "postgres" | "postgresql" | "cockroachdb" | "redshift" => {
            let schema = schema_opt.unwrap_or_else(|| "public".to_string());
            let sql = "SELECT
                    kcu.column_name,
                    CASE WHEN ccu.table_schema = 'public' THEN ccu.table_name
                         ELSE ccu.table_schema || '.' || ccu.table_name END AS foreign_table,
                    ccu.column_name AS foreign_column,
                    tc.constraint_name
                FROM information_schema.table_constraints AS tc
                JOIN information_schema.key_column_usage AS kcu
                  ON tc.constraint_name = kcu.constraint_name
                 AND tc.table_schema = kcu.table_schema
                JOIN information_schema.constraint_column_usage AS ccu
                  ON ccu.constraint_name = tc.constraint_name
                 AND ccu.table_schema = tc.table_schema
                WHERE tc.constraint_type = 'FOREIGN KEY'
                  AND tc.table_schema = $2
                  AND tc.table_name = $1";
            let rows = sqlx::query::<Any>(sql)
                .bind(&bare_table)
                .bind(&schema)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            for row in rows {
                fks.push(ForeignKeyInfo {
                    column_name: row.get(0),
                    referenced_table: row.get(1),
                    referenced_column: row.get(2),
                    constraint_name: row.get(3),
                });
            }
        }
        "mysql" | "mariadb" => {
            let rows = if let Some(ref schema) = schema_opt {
                let sql = "SELECT COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME, CONSTRAINT_NAME
                    FROM information_schema.KEY_COLUMN_USAGE
                    WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ? AND REFERENCED_TABLE_NAME IS NOT NULL";
                sqlx::query::<Any>(sql)
                    .bind(schema)
                    .bind(&bare_table)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| e.to_string())?
            } else {
                let sql = "SELECT COLUMN_NAME, REFERENCED_TABLE_NAME, REFERENCED_COLUMN_NAME, CONSTRAINT_NAME
                    FROM information_schema.KEY_COLUMN_USAGE
                    WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ? AND REFERENCED_TABLE_NAME IS NOT NULL";
                sqlx::query::<Any>(sql)
                    .bind(&bare_table)
                    .fetch_all(pool)
                    .await
                    .map_err(|e| e.to_string())?
            };
            for row in rows {
                fks.push(ForeignKeyInfo {
                    column_name: row.get(0),
                    referenced_table: row.get(1),
                    referenced_column: row.get(2),
                    constraint_name: row.get(3),
                });
            }
        }
        _ => {
            let safe = bare_table.replace('"', "\"\"");
            let sql = format!("PRAGMA foreign_key_list(\"{}\");", safe);
            let rows = sqlx::query::<Any>(&sql)
                .fetch_all(pool)
                .await
                .map_err(|e| e.to_string())?;
            for row in rows {
                let ref_table: String = row.get(2);
                let from_col: String = row.get(3);
                let to_col: String = row.get(4);
                fks.push(ForeignKeyInfo {
                    column_name: from_col,
                    referenced_table: ref_table,
                    referenced_column: to_col,
                    constraint_name: String::new(),
                });
            }
        }
    }

    Ok(fks)
}

// Analyze primary key constraints on a table to enforce read-only safety
pub fn analyze_primary_keys(columns: &[ColumnInfo]) -> PkAnalysis {
    let pk_cols: Vec<&ColumnInfo> = columns.iter().filter(|c| c.is_primary_key).collect();
    
    if pk_cols.len() == 1 {
        PkAnalysis {
            has_single_pk: true,
            pk_column_name: Some(pk_cols[0].name.clone()),
            pk_columns: vec![pk_cols[0].name.clone()],
            is_read_only: false,
            read_only_reason: None,
        }
    } else if pk_cols.is_empty() {
        // SQLite rowid exception handling
        PkAnalysis {
            has_single_pk: false,
            pk_column_name: Some("rowid".to_string()),
            pk_columns: vec!["rowid".to_string()],
            is_read_only: false,
            read_only_reason: Some("Using SQLite rowid as primary key fallback.".to_string()),
        }
    } else {
        // Table has composite (multi-column) primary keys - now FULLY EDITABLE!
        let names: Vec<String> = pk_cols.iter().map(|c| c.name.clone()).collect();
        PkAnalysis {
            has_single_pk: false,
            pk_column_name: Some(names.join(", ")),
            pk_columns: names,
            is_read_only: false,
            read_only_reason: None,
        }
    }
}

#[cfg(test)] // Conditional compilation attribute for unit tests
mod tests { // Declare internal unit testing module
    use super::*; // Import outer module items into test scope

    #[test] // Mark test function for single primary key validation
    fn test_single_pk_analysis() { // Test single PK analysis behavior
        let cols = vec![ // Create mock columns vector
            ColumnInfo {
                name: "id".to_string(),
                data_type: "INT".to_string(),
                is_nullable: false,
                is_primary_key: true,
                is_foreign_key: false,
                fk_table: None,
                fk_column: None,
            },
            ColumnInfo {
                name: "val".to_string(),
                data_type: "TEXT".to_string(),
                is_nullable: true,
                is_primary_key: false,
                is_foreign_key: false,
                fk_table: None,
                fk_column: None,
            },
        ];
        let res = analyze_primary_keys(&cols); // Run primary key analysis function
        assert!(res.has_single_pk); // Assert single PK flag is true
        assert!(!res.is_read_only); // Assert read-only flag is false
        assert_eq!(res.pk_column_name, Some("id".to_string())); // Assert primary key column name matches "id"
    } // End of test_single_pk_analysis test function

    #[test] // Mark test function for zero primary key validation (SQLite rowid fallback)
    fn test_no_pk_analysis_rowid_fallback() { // Test zero PK analysis behavior
        let cols = vec![
            ColumnInfo {
                name: "val".to_string(),
                data_type: "TEXT".to_string(),
                is_nullable: true,
                is_primary_key: false,
                is_foreign_key: false,
                fk_table: None,
                fk_column: None,
            },
        ];
        let res = analyze_primary_keys(&cols); // Run primary key analysis function
        assert!(!res.has_single_pk); // Assert single PK flag is false
        assert!(!res.is_read_only); // Assert rowid fallback makes it editable
        assert_eq!(res.pk_column_name, Some("rowid".to_string())); // Assert rowid fallback
    }

    #[tokio::test]
    async fn test_sqlite_foreign_key_enrichment() {
        sqlx::any::install_default_drivers();
        use sqlx::any::AnyPoolOptions;
        use sqlx::Executor;
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        pool.execute("PRAGMA foreign_keys = ON;").await.unwrap();
        pool.execute("CREATE TABLE parent (id INTEGER PRIMARY KEY);")
            .await
            .unwrap();
        pool.execute(
            "CREATE TABLE child (id INTEGER PRIMARY KEY, parent_id INTEGER REFERENCES parent(id));",
        )
        .await
        .unwrap();
        let cols = fetch_columns(&pool, "sqlite", "child").await.unwrap();
        let parent_col = cols.iter().find(|c| c.name == "parent_id").unwrap();
        assert!(parent_col.is_foreign_key);
        assert_eq!(parent_col.fk_table.as_deref(), Some("parent"));
        assert_eq!(parent_col.fk_column.as_deref(), Some("id"));
    }

    #[tokio::test]
    async fn test_sqlite_fetch_tables_includes_views() {
        sqlx::any::install_default_drivers();
        use sqlx::any::AnyPoolOptions;
        use sqlx::Executor;
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        pool.execute("CREATE TABLE t1 (id INTEGER PRIMARY KEY);")
            .await
            .unwrap();
        pool.execute("CREATE VIEW v1 AS SELECT id FROM t1;")
            .await
            .unwrap();
        let tables = fetch_tables(&pool, "sqlite").await.unwrap();
        assert!(tables.iter().any(|t| t.name == "t1" && t.table_type.contains("TABLE")));
        assert!(tables.iter().any(|t| t.name == "v1" && t.table_type == "VIEW"));
        assert!(tables.iter().all(|t| !t.schema.is_empty()));
    }

    #[test]
    fn test_split_schema_table() {
        assert_eq!(
            split_schema_table("public.users"),
            (Some("public".into()), "users".into())
        );
        assert_eq!(split_schema_table("users"), (None, "users".into()));
        assert_eq!(
            split_schema_table("analytics.events"),
            (Some("analytics".into()), "events".into())
        );
    }
} // End of tests module
