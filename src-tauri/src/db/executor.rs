// Dynamic SQL query execution engine and row-to-JSON encoder
use serde::{Deserialize, Serialize}; // Import Serde traits for JSON object serialization
use serde_json::{json, Value}; // Import Serde JSON value enum and construction macro
use sqlx::any::AnyRow; // Import AnyRow dynamic row type
use sqlx::postgres::{PgConnection, PgRow};
use sqlx::mysql::{MySqlConnection, MySqlRow};
use sqlx::AnyPool; // Import AnyPool from sqlx root (correct for 0.8)
use sqlx::{Column, Row, TypeInfo}; // Import Column, Row, and TypeInfo traits from sqlx
use std::time::Instant; // Import Instant struct from standard library for timing query duration
use futures_util::TryStreamExt; // For tiberius query streams

// Data structure representing a single column header descriptor in query result
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Derive common standard traits
pub struct ColumnHeader { // Struct definition for column header
    pub name: String, // Column header title name
    pub type_name: String, // Column data type string representation
} // End of ColumnHeader struct definition

// Data structure representing complete executed query result payload
#[derive(Debug, Serialize, Deserialize, Clone)] // Derive traits for JSON encoding
pub struct QueryResultPayload { // Struct definition for query result response
    pub columns: Vec<ColumnHeader>, // Array of column headers in result set
    pub rows: Vec<Vec<Value>>, // Matrix array of rows containing JSON cell values
    pub execution_time_ms: u64, // Query execution duration in milliseconds
    pub affected_rows: u64, // Number of database rows modified or returned
} // End of QueryResultPayload struct definition

// Convert dynamic sqlx AnyRow column cell value into generic serde_json::Value
// Note: sqlx::Any only decodes a limited set of types (not chrono DateTime).
pub fn decode_any_cell(row: &AnyRow, index: usize) -> Value {
    // Try string representation first as most database values convert to string easily
    if let Ok(val) = row.try_get::<String, _>(index) {
        // Attempt to parse JSON string if it looks like structured JSON
        if (val.starts_with('{') && val.ends_with('}')) || (val.starts_with('[') && val.ends_with(']')) {
            if let Ok(json_val) = serde_json::from_str::<Value>(&val) {
                return json_val;
            }
        }
        Value::String(val)
    } else if let Ok(val) = row.try_get::<i64, _>(index) {
        json!(val)
    } else if let Ok(val) = row.try_get::<i32, _>(index) {
        json!(val)
    } else if let Ok(val) = row.try_get::<f64, _>(index) {
        json!(val)
    } else if let Ok(val) = row.try_get::<bool, _>(index) {
        Value::Bool(val)
    } else if let Ok(val) = row.try_get::<Vec<u8>, _>(index) {
        // Binary / BLOB — surface as base64 so the UI can inspect without loss
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        Value::String(STANDARD.encode(val))
    } else {
        Value::Null
    }
}

pub fn decode_pg_cell(row: &PgRow, index: usize) -> Value {
    use sqlx::ValueRef;
    if row.try_get_raw(index).map(|v| v.is_null()).unwrap_or(false) {
        return Value::Null;
    }

    if let Ok(val) = row.try_get::<String, _>(index) {
        if (val.starts_with('{') && val.ends_with('}')) || (val.starts_with('[') && val.ends_with(']')) {
            if let Ok(json_val) = serde_json::from_str::<Value>(&val) {
                return json_val;
            }
        }
        return Value::String(val);
    }
    if let Ok(val) = row.try_get::<serde_json::Value, _>(index) {
        return val;
    }
    if let Ok(val) = row.try_get::<i64, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<i32, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<i16, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<f64, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<f32, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<bool, _>(index) {
        return Value::Bool(val);
    }
    if let Ok(val) = row.try_get::<uuid::Uuid, _>(index) {
        return Value::String(val.to_string());
    }
    if let Ok(val) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(index) {
        return Value::String(val.to_rfc3339());
    }
    if let Ok(val) = row.try_get::<chrono::NaiveDateTime, _>(index) {
        return Value::String(val.to_string());
    }
    if let Ok(val) = row.try_get::<chrono::NaiveDate, _>(index) {
        return Value::String(val.to_string());
    }
    if let Ok(val) = row.try_get::<chrono::NaiveTime, _>(index) {
        return Value::String(val.to_string());
    }
    if let Ok(val) = row.try_get::<rust_decimal::Decimal, _>(index) {
        return Value::String(val.to_string());
    }
    if let Ok(val) = row.try_get::<Vec<String>, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<Vec<i64>, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<Vec<i32>, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<Vec<f64>, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<Vec<bool>, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<Vec<serde_json::Value>, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<Vec<u8>, _>(index) {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        return Value::String(STANDARD.encode(val));
    }
    Value::Null
}

pub fn decode_mysql_cell(row: &MySqlRow, index: usize) -> Value {
    use sqlx::ValueRef;
    if row.try_get_raw(index).map(|v| v.is_null()).unwrap_or(false) {
        return Value::Null;
    }

    if let Ok(val) = row.try_get::<String, _>(index) {
        if (val.starts_with('{') && val.ends_with('}')) || (val.starts_with('[') && val.ends_with(']')) {
            if let Ok(json_val) = serde_json::from_str::<Value>(&val) {
                return json_val;
            }
        }
        return Value::String(val);
    }
    if let Ok(val) = row.try_get::<serde_json::Value, _>(index) {
        return val;
    }
    if let Ok(val) = row.try_get::<i64, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<i32, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<i16, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<f64, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<f32, _>(index) {
        return json!(val);
    }
    if let Ok(val) = row.try_get::<bool, _>(index) {
        return Value::Bool(val);
    }
    if let Ok(val) = row.try_get::<chrono::NaiveDateTime, _>(index) {
        return Value::String(val.to_string());
    }
    if let Ok(val) = row.try_get::<chrono::NaiveDate, _>(index) {
        return Value::String(val.to_string());
    }
    if let Ok(val) = row.try_get::<rust_decimal::Decimal, _>(index) {
        return Value::String(val.to_string());
    }
    if let Ok(val) = row.try_get::<Vec<u8>, _>(index) {
        use base64::{engine::general_purpose::STANDARD, Engine as _};
        return Value::String(STANDARD.encode(val));
    }
    Value::Null
}

/// Returns true when the statement is expected to return a result set.
fn expects_result_set(sql: &str) -> bool {
    let trimmed = sql.trim_start();
    // Strip leading SQL comments (single-line)
    let mut s = trimmed;
    loop {
        if s.starts_with("--") {
            if let Some(pos) = s.find('\n') {
                s = s[pos + 1..].trim_start();
                continue;
            }
            return false;
        }
        if s.starts_with("/*") {
            if let Some(pos) = s.find("*/") {
                s = s[pos + 2..].trim_start();
                continue;
            }
            return false;
        }
        break;
    }
    let upper = s.to_uppercase();
    upper.starts_with("SELECT")
        || upper.starts_with("WITH")
        || upper.starts_with("SHOW")
        || upper.starts_with("DESCRIBE")
        || upper.starts_with("DESC ")
        || upper.starts_with("EXPLAIN")
        || upper.starts_with("PRAGMA")
        || upper.starts_with("VALUES")
}

// Execute arbitrary dynamic SQL string against connection pool and return formatted payload
pub async fn execute_dynamic_query(pool: &AnyPool, sql: &str) -> Result<QueryResultPayload, String> {
    let start_time = Instant::now();

    // DML/DDL statements (INSERT/UPDATE/DELETE/CREATE/...) should use execute(),
    // not fetch_all(). Using fetch_all on non-SELECT queries fails on most drivers.
    if !expects_result_set(sql) {
        let result = sqlx::query(sql)
            .execute(pool)
            .await
            .map_err(|e| format!("Query execution failed: {}", e))?;

        return Ok(QueryResultPayload {
            columns: vec![ColumnHeader {
                name: "affected_rows".to_string(),
                type_name: "INTEGER".to_string(),
            }],
            rows: vec![vec![json!(result.rows_affected())]],
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            affected_rows: result.rows_affected(),
        });
    }

    let rows: Vec<AnyRow> = sqlx::query(sql)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Query execution failed: {}", e))?;

    Ok(format_fetched_rows(rows, start_time))
}

/// Execute on a held pool connection (for open transactions).
pub async fn execute_dynamic_query_on_connection(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Any>,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let start_time = Instant::now();

    if !expects_result_set(sql) {
        let result = sqlx::query(sql)
            .execute(&mut **conn)
            .await
            .map_err(|e| format!("Query execution failed: {}", e))?;

        return Ok(QueryResultPayload {
            columns: vec![ColumnHeader {
                name: "affected_rows".to_string(),
                type_name: "INTEGER".to_string(),
            }],
            rows: vec![vec![json!(result.rows_affected())]],
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            affected_rows: result.rows_affected(),
        });
    }

    let rows: Vec<AnyRow> = sqlx::query(sql)
        .fetch_all(&mut **conn)
        .await
        .map_err(|e| format!("Query execution failed: {}", e))?;

    Ok(format_fetched_rows(rows, start_time))
}

fn format_fetched_rows(rows: Vec<AnyRow>, start_time: Instant) -> QueryResultPayload {
    let mut columns = Vec::new();
    let mut result_rows = Vec::new();

    if let Some(first_row) = rows.first() {
        for col in first_row.columns() {
            columns.push(ColumnHeader {
                name: col.name().to_string(),
                type_name: col.type_info().name().to_string(),
            });
        }
    }

    for row in &rows {
        let mut row_values = Vec::new();
        for i in 0..row.columns().len() {
            row_values.push(decode_any_cell(row, i));
        }
        result_rows.push(row_values);
    }

    let affected_rows = rows.len() as u64;
    QueryResultPayload {
        columns,
        rows: result_rows,
        execution_time_ms: start_time.elapsed().as_millis() as u64,
        affected_rows,
    }
}

// Chunked stream payload for emitting partial query row blocks
#[cfg(feature = "gui")]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamChunkPayload {
    pub query_id: String,
    pub chunk_index: usize,
    pub rows: Vec<Vec<Value>>,
}

// Event payload emitted when dynamic query stream finishes
#[cfg(feature = "gui")]
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StreamDonePayload {
    pub query_id: String,
    pub execution_time_ms: u64,
    pub total_rows: u64,
}

/// Best-effort backend session id used for protocol-level cancel (PG/MySQL).
pub async fn fetch_backend_pid(
    conn: &mut sqlx::pool::PoolConnection<sqlx::Any>,
    db_kind: &str,
) -> Option<u32> {
    let sql = match db_kind.to_lowercase().as_str() {
        "postgres" | "postgresql" | "cockroachdb" | "redshift" => "SELECT pg_backend_pid()",
        "mysql" | "mariadb" => "SELECT CONNECTION_ID()",
        _ => return None,
    };
    let row = sqlx::query(sql).fetch_one(&mut **conn).await.ok()?;
    if let Ok(v) = row.try_get::<i64, _>(0) {
        return Some(v as u32);
    }
    if let Ok(v) = row.try_get::<i32, _>(0) {
        return Some(v as u32);
    }
    None
}

// GAP 12: Protocol-level Backend Process Termination
pub async fn cancel_backend_process(pool: &AnyPool, pid_or_thread_id: u32, db_kind: &str) -> Result<(), String> {
    let cancel_sql = match db_kind.to_lowercase().as_str() {
        "postgres" | "postgresql" | "cockroachdb" | "redshift" => {
            format!("SELECT pg_cancel_backend({})", pid_or_thread_id)
        }
        "mysql" | "mariadb" => format!("KILL QUERY {}", pid_or_thread_id),
        "mssql" => format!("KILL {}", pid_or_thread_id),
        _ => {
            return Err(format!(
                "Engine {} does not support remote protocol query cancellation",
                db_kind
            ))
        }
    };
    sqlx::query(&cancel_sql)
        .execute(pool)
        .await
        .map_err(|e| format!("Protocol query cancellation failed: {}", e))?;
    Ok(())
}

// Stream dynamic query results in chunks of chunk_size (default 500) to prevent RAM bloating
#[cfg(feature = "gui")]
pub async fn stream_dynamic_query<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    pool: &AnyPool,
    query_id: &str,
    sql: &str,
    chunk_size: usize,
) -> Result<QueryResultPayload, String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    let start_time = Instant::now();
    let mut stream = sqlx::query(sql).fetch(pool);

    let mut columns = Vec::new();
    let mut current_chunk: Vec<Vec<Value>> = Vec::with_capacity(chunk_size);
    let mut total_rows: u64 = 0;
    let mut chunk_index: usize = 0;
    let mut header_emitted = false;

    while let Some(row_res) = stream.next().await {
        let row = row_res.map_err(|e| format!("Query stream error: {}", e))?;

        if !header_emitted {
            for col in row.columns() {
                columns.push(ColumnHeader {
                    name: col.name().to_string(),
                    type_name: col.type_info().name().to_string(),
                });
            }
            let _ = app_handle.emit(&format!("query_columns_{}", query_id), &columns);
            header_emitted = true;
        }

        let mut row_values = Vec::with_capacity(row.columns().len());
        for i in 0..row.columns().len() {
            row_values.push(decode_any_cell(&row, i));
        }

        current_chunk.push(row_values);
        total_rows += 1;

        if current_chunk.len() >= chunk_size {
            let chunk_payload = StreamChunkPayload {
                query_id: query_id.to_string(),
                chunk_index,
                rows: std::mem::replace(&mut current_chunk, Vec::with_capacity(chunk_size)),
            };
            let _ = app_handle.emit(&format!("query_chunk_{}", query_id), &chunk_payload);
            chunk_index += 1;
        }
    }

    if !current_chunk.is_empty() {
        let chunk_payload = StreamChunkPayload {
            query_id: query_id.to_string(),
            chunk_index,
            rows: current_chunk,
        };
        let _ = app_handle.emit(&format!("query_chunk_{}", query_id), &chunk_payload);
    }

    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    let done_payload = StreamDonePayload {
        query_id: query_id.to_string(),
        execution_time_ms,
        total_rows,
    };
    let _ = app_handle.emit(&format!("query_done_{}", query_id), &done_payload);

    Ok(QueryResultPayload {
        columns,
        rows: Vec::new(),
        execution_time_ms,
        affected_rows: total_rows,
    })
}

// ── Native Postgres and MySQL Execution Drivers ─────────────────────────────

pub async fn execute_pg_query(
    pool: &sqlx::PgPool,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let start_time = Instant::now();

    if !expects_result_set(sql) {
        let result = sqlx::query(sql)
            .execute(pool)
            .await
            .map_err(|e| format!("Query execution failed: {}", e))?;

        return Ok(QueryResultPayload {
            columns: vec![ColumnHeader {
                name: "affected_rows".to_string(),
                type_name: "INTEGER".to_string(),
            }],
            rows: vec![vec![json!(result.rows_affected())]],
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            affected_rows: result.rows_affected(),
        });
    }

    let rows: Vec<PgRow> = sqlx::query(sql)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Query execution failed: {}", e))?;

    let mut columns = Vec::new();
    let mut result_rows = Vec::new();

    if let Some(first_row) = rows.first() {
        for col in first_row.columns() {
            columns.push(ColumnHeader {
                name: col.name().to_string(),
                type_name: col.type_info().name().to_string(),
            });
        }
    }

    for row in &rows {
        let mut row_values = Vec::new();
        for i in 0..row.columns().len() {
            row_values.push(decode_pg_cell(row, i));
        }
        result_rows.push(row_values);
    }

    let affected_rows = rows.len() as u64;
    Ok(QueryResultPayload {
        columns,
        rows: result_rows,
        execution_time_ms: start_time.elapsed().as_millis() as u64,
        affected_rows,
    })
}

pub async fn execute_pg_query_on_conn(
    conn: &mut PgConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let start_time = Instant::now();

    if !expects_result_set(sql) {
        let result = sqlx::query(sql)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("Query execution failed: {}", e))?;

        return Ok(QueryResultPayload {
            columns: vec![ColumnHeader {
                name: "affected_rows".to_string(),
                type_name: "INTEGER".to_string(),
            }],
            rows: vec![vec![json!(result.rows_affected())]],
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            affected_rows: result.rows_affected(),
        });
    }

    let rows: Vec<PgRow> = sqlx::query(sql)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| format!("Query execution failed: {}", e))?;

    let mut columns = Vec::new();
    let mut result_rows = Vec::new();

    if let Some(first_row) = rows.first() {
        for col in first_row.columns() {
            columns.push(ColumnHeader {
                name: col.name().to_string(),
                type_name: col.type_info().name().to_string(),
            });
        }
    }

    for row in &rows {
        let mut row_values = Vec::new();
        for i in 0..row.columns().len() {
            row_values.push(decode_pg_cell(row, i));
        }
        result_rows.push(row_values);
    }

    let affected_rows = rows.len() as u64;
    Ok(QueryResultPayload {
        columns,
        rows: result_rows,
        execution_time_ms: start_time.elapsed().as_millis() as u64,
        affected_rows,
    })
}

#[cfg(feature = "gui")]
pub async fn stream_pg_query<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    pool: &sqlx::PgPool,
    query_id: &str,
    sql: &str,
    chunk_size: usize,
) -> Result<QueryResultPayload, String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    let start_time = Instant::now();
    let mut stream = sqlx::query(sql).fetch(pool);

    let mut columns = Vec::new();
    let mut current_chunk: Vec<Vec<Value>> = Vec::with_capacity(chunk_size);
    let mut total_rows: u64 = 0;
    let mut chunk_index: usize = 0;
    let mut header_emitted = false;

    while let Some(row_res) = stream.next().await {
        let row = row_res.map_err(|e| format!("Query stream error: {}", e))?;

        if !header_emitted {
            for col in row.columns() {
                columns.push(ColumnHeader {
                    name: col.name().to_string(),
                    type_name: col.type_info().name().to_string(),
                });
            }
            let _ = app_handle.emit(&format!("query_columns_{}", query_id), &columns);
            header_emitted = true;
        }

        let mut row_values = Vec::with_capacity(row.columns().len());
        for i in 0..row.columns().len() {
            row_values.push(decode_pg_cell(&row, i));
        }

        current_chunk.push(row_values);
        total_rows += 1;

        if current_chunk.len() >= chunk_size {
            let chunk_payload = StreamChunkPayload {
                query_id: query_id.to_string(),
                chunk_index,
                rows: std::mem::replace(&mut current_chunk, Vec::with_capacity(chunk_size)),
            };
            let _ = app_handle.emit(&format!("query_chunk_{}", query_id), &chunk_payload);
            chunk_index += 1;
        }
    }

    if !current_chunk.is_empty() {
        let chunk_payload = StreamChunkPayload {
            query_id: query_id.to_string(),
            chunk_index,
            rows: current_chunk,
        };
        let _ = app_handle.emit(&format!("query_chunk_{}", query_id), &chunk_payload);
    }

    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    let done_payload = StreamDonePayload {
        query_id: query_id.to_string(),
        execution_time_ms,
        total_rows,
    };
    let _ = app_handle.emit(&format!("query_done_{}", query_id), &done_payload);

    Ok(QueryResultPayload {
        columns,
        rows: Vec::new(),
        execution_time_ms,
        affected_rows: total_rows,
    })
}

pub async fn execute_mysql_query(
    pool: &sqlx::MySqlPool,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let start_time = Instant::now();

    if !expects_result_set(sql) {
        let result = sqlx::query(sql)
            .execute(pool)
            .await
            .map_err(|e| format!("Query execution failed: {}", e))?;

        return Ok(QueryResultPayload {
            columns: vec![ColumnHeader {
                name: "affected_rows".to_string(),
                type_name: "INTEGER".to_string(),
            }],
            rows: vec![vec![json!(result.rows_affected())]],
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            affected_rows: result.rows_affected(),
        });
    }

    let rows: Vec<MySqlRow> = sqlx::query(sql)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Query execution failed: {}", e))?;

    let mut columns = Vec::new();
    let mut result_rows = Vec::new();

    if let Some(first_row) = rows.first() {
        for col in first_row.columns() {
            columns.push(ColumnHeader {
                name: col.name().to_string(),
                type_name: col.type_info().name().to_string(),
            });
        }
    }

    for row in &rows {
        let mut row_values = Vec::new();
        for i in 0..row.columns().len() {
            row_values.push(decode_mysql_cell(row, i));
        }
        result_rows.push(row_values);
    }

    let affected_rows = rows.len() as u64;
    Ok(QueryResultPayload {
        columns,
        rows: result_rows,
        execution_time_ms: start_time.elapsed().as_millis() as u64,
        affected_rows,
    })
}

pub async fn execute_mysql_query_on_conn(
    conn: &mut MySqlConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let start_time = Instant::now();

    if !expects_result_set(sql) {
        let result = sqlx::query(sql)
            .execute(&mut *conn)
            .await
            .map_err(|e| format!("Query execution failed: {}", e))?;

        return Ok(QueryResultPayload {
            columns: vec![ColumnHeader {
                name: "affected_rows".to_string(),
                type_name: "INTEGER".to_string(),
            }],
            rows: vec![vec![json!(result.rows_affected())]],
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            affected_rows: result.rows_affected(),
        });
    }

    let rows: Vec<MySqlRow> = sqlx::query(sql)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| format!("Query execution failed: {}", e))?;

    let mut columns = Vec::new();
    let mut result_rows = Vec::new();

    if let Some(first_row) = rows.first() {
        for col in first_row.columns() {
            columns.push(ColumnHeader {
                name: col.name().to_string(),
                type_name: col.type_info().name().to_string(),
            });
        }
    }

    for row in &rows {
        let mut row_values = Vec::new();
        for i in 0..row.columns().len() {
            row_values.push(decode_mysql_cell(row, i));
        }
        result_rows.push(row_values);
    }

    let affected_rows = rows.len() as u64;
    Ok(QueryResultPayload {
        columns,
        rows: result_rows,
        execution_time_ms: start_time.elapsed().as_millis() as u64,
        affected_rows,
    })
}

#[cfg(feature = "gui")]
pub async fn stream_mysql_query<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    pool: &sqlx::MySqlPool,
    query_id: &str,
    sql: &str,
    chunk_size: usize,
) -> Result<QueryResultPayload, String> {
    use futures_util::StreamExt;
    use tauri::Emitter;

    let start_time = Instant::now();
    let mut stream = sqlx::query(sql).fetch(pool);

    let mut columns = Vec::new();
    let mut current_chunk: Vec<Vec<Value>> = Vec::with_capacity(chunk_size);
    let mut total_rows: u64 = 0;
    let mut chunk_index: usize = 0;
    let mut header_emitted = false;

    while let Some(row_res) = stream.next().await {
        let row = row_res.map_err(|e| format!("Query stream error: {}", e))?;

        if !header_emitted {
            for col in row.columns() {
                columns.push(ColumnHeader {
                    name: col.name().to_string(),
                    type_name: col.type_info().name().to_string(),
                });
            }
            let _ = app_handle.emit(&format!("query_columns_{}", query_id), &columns);
            header_emitted = true;
        }

        let mut row_values = Vec::with_capacity(row.columns().len());
        for i in 0..row.columns().len() {
            row_values.push(decode_mysql_cell(&row, i));
        }

        current_chunk.push(row_values);
        total_rows += 1;

        if current_chunk.len() >= chunk_size {
            let chunk_payload = StreamChunkPayload {
                query_id: query_id.to_string(),
                chunk_index,
                rows: std::mem::replace(&mut current_chunk, Vec::with_capacity(chunk_size)),
            };
            let _ = app_handle.emit(&format!("query_chunk_{}", query_id), &chunk_payload);
            chunk_index += 1;
        }
    }

    if !current_chunk.is_empty() {
        let chunk_payload = StreamChunkPayload {
            query_id: query_id.to_string(),
            chunk_index,
            rows: current_chunk,
        };
        let _ = app_handle.emit(&format!("query_chunk_{}", query_id), &chunk_payload);
    }

    let execution_time_ms = start_time.elapsed().as_millis() as u64;

    let done_payload = StreamDonePayload {
        query_id: query_id.to_string(),
        execution_time_ms,
        total_rows,
    };
    let _ = app_handle.emit(&format!("query_done_{}", query_id), &done_payload);

    Ok(QueryResultPayload {
        columns,
        rows: Vec::new(),
        execution_time_ms,
        affected_rows: total_rows,
    })
}

pub async fn execute_query_for_managed(
    conn: &ManagedConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let db_type = conn.db_type.to_lowercase();
    if matches!(db_type.as_str(), "postgres" | "postgresql" | "cockroachdb" | "redshift") {
        if let Some(ref pg_pool) = conn.pg_pool {
            return execute_pg_query(pg_pool, sql).await;
        }
    } else if matches!(db_type.as_str(), "mysql" | "mariadb") {
        if let Some(ref mysql_pool) = conn.mysql_pool {
            return execute_mysql_query(mysql_pool, sql).await;
        }
    } else if db_type == "mssql" || db_type == "sqlserver" {
        return execute_mssql_query(conn, sql).await;
    } else if db_type == "mongodb" {
        return execute_mongo_query(conn, sql).await;
    } else if db_type == "redis" {
        return execute_redis_query(conn, sql).await;
    } else if db_type == "cassandra" {
        return execute_scylla_query(conn, sql).await;
    } else if db_type == "clickhouse" {
        return execute_clickhouse_query(conn, sql).await;
    } else if db_type == "duckdb" {
        return execute_duckdb_query(conn, sql).await;
    } else if db_type == "turso" {
        return execute_libsql_query(conn, sql).await;
    } else if db_type == "snowflake" {
        return execute_snowflake_query(conn, sql).await;
    } else if db_type == "oracle" {
        return execute_oracle_query(conn, sql).await;
    }
    execute_dynamic_query(&conn.pool, sql).await
}

#[cfg(feature = "gui")]
pub async fn stream_query_for_managed<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    conn: &ManagedConnection,
    query_id: &str,
    sql: &str,
    chunk_size: usize,
) -> Result<QueryResultPayload, String> {
    let db_type = conn.db_type.to_lowercase();
    if matches!(db_type.as_str(), "postgres" | "postgresql" | "cockroachdb" | "redshift") {
        if let Some(ref pg_pool) = conn.pg_pool {
            return stream_pg_query(app_handle, pg_pool, query_id, sql, chunk_size).await;
        }
    } else if matches!(db_type.as_str(), "mysql" | "mariadb") {
        if let Some(ref mysql_pool) = conn.mysql_pool {
            return stream_mysql_query(app_handle, mysql_pool, query_id, sql, chunk_size).await;
        }
    } else if db_type == "mssql" || db_type == "sqlserver" {
        return stream_mssql_query(app_handle, conn, query_id, sql, chunk_size).await;
    }
    stream_dynamic_query(app_handle, &conn.pool, query_id, sql, chunk_size).await
}

#[cfg(test)] // Conditional compilation attribute for unit test module
mod tests { // Declare internal unit testing module
    use super::*; // Import parent module items into test scope

    #[test] // Mark function as unit test for ColumnHeader struct
    fn test_column_header_structure() { // Test function verifying ColumnHeader serialization
        let col = ColumnHeader { name: "id".to_string(), type_name: "INTEGER".to_string() }; // Instantiate ColumnHeader
        assert_eq!(col.name, "id"); // Assert name field matches expected value
        assert_eq!(col.type_name, "INTEGER"); // Assert type_name field matches expected value
    } // End of test function

    #[test]
    fn test_split_redis_args_quoted_and_spaces() {
        let args = split_redis_args("SET 'my key' \"hello world\" 100");
        assert_eq!(args, vec!["SET", "my key", "hello world", "100"]);
    }
} // End of tests module

#[cfg(test)]
mod integration_tests {
    use super::*;
    use sqlx::any::AnyPoolOptions;

    #[tokio::test]
    async fn test_dml_and_select_on_sqlite() {
        sqlx::any::install_default_drivers();
        // Single connection: in-memory SQLite is not shared across pool connections.
        let pool = AnyPoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect");

        let create = execute_dynamic_query(
            &pool,
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);",
        )
        .await
        .expect("create");
        assert_eq!(create.columns[0].name, "affected_rows");

        let insert = execute_dynamic_query(
            &pool,
            "INSERT INTO users (id, name) VALUES (1, 'Ada');",
        )
        .await
        .expect("insert");
        assert_eq!(insert.affected_rows, 1);

        let select = execute_dynamic_query(&pool, "SELECT id, name FROM users;")
            .await
            .expect("select");
        assert_eq!(select.rows.len(), 1);
        assert_eq!(select.rows[0][1], serde_json::json!("Ada"));
    }
}

use crate::db::pool::ManagedConnection;
use tiberius::QueryItem;

pub async fn execute_mssql_query_on_conn(
    conn: &mut tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let start_time = Instant::now();

    if !expects_result_set(sql) {
        let result = conn
            .execute(sql, &[])
            .await
            .map_err(|e| format!("MSSQL execution failed: {}", e))?;

        let affected = result.total() as u64;
        return Ok(QueryResultPayload {
            columns: vec![ColumnHeader {
                name: "affected_rows".to_string(),
                type_name: "INTEGER".to_string(),
            }],
            rows: vec![vec![json!(affected)]],
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            affected_rows: affected,
        });
    }

    let mut stream = conn
        .simple_query(sql)
        .await
        .map_err(|e| format!("MSSQL execution failed: {}", e))?;
    
    let mut columns = Vec::new();
    let mut rows = Vec::new();
    let mut total_rows = 0;

    while let Some(item) = stream.try_next().await.map_err(|e| e.to_string())? {
        match item {
            QueryItem::Metadata(metadata) => {
                if columns.is_empty() {
                    for col in metadata.columns() {
                        columns.push(ColumnHeader {
                            name: col.name().to_string(),
                            type_name: format!("{:?}", col.column_type()),
                        });
                    }
                }
            }
            QueryItem::Row(row) => {
                let mut row_values = Vec::new();
                for i in 0..row.columns().len() {
                    row_values.push(decode_mssql_cell(&row, i));
                }
                rows.push(row_values);
                total_rows += 1;
            }
        }
    }

    Ok(QueryResultPayload {
        columns,
        rows,
        execution_time_ms: start_time.elapsed().as_millis() as u64,
        affected_rows: total_rows,
    })
}

pub async fn execute_mssql_query(
    managed_conn: &ManagedConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let pool = managed_conn
        .mssql_pool
        .as_ref()
        .ok_or_else(|| "MSSQL pool not initialized".to_string())?;

    let mut conn = pool
        .get()
        .await
        .map_err(|e| format!("Failed to acquire MSSQL connection: {}", e))?;

    execute_mssql_query_on_conn(&mut *conn, sql).await
}

#[cfg(feature = "gui")]
pub async fn stream_mssql_query<R: tauri::Runtime>(
    app_handle: &tauri::AppHandle<R>,
    managed_conn: &ManagedConnection,
    query_id: &str,
    sql: &str,
    chunk_size: usize,
) -> Result<QueryResultPayload, String> {
    use tauri::Emitter;
    let pool = managed_conn
        .mssql_pool
        .as_ref()
        .ok_or_else(|| "MSSQL pool not initialized".to_string())?;

    let mut conn = pool
        .get()
        .await
        .map_err(|e| format!("Failed to acquire MSSQL connection: {}", e))?;

    let start_time = Instant::now();

    let mut stream = conn
        .simple_query(sql)
        .await
        .map_err(|e| format!("MSSQL execution failed: {}", e))?;
    
    let mut columns = Vec::new();
    let mut chunk = Vec::new();
    let mut total_rows = 0;

    while let Some(item) = stream.try_next().await.map_err(|e| e.to_string())? {
        match item {
            QueryItem::Metadata(metadata) => {
                if columns.is_empty() {
                    for col in metadata.columns() {
                        columns.push(ColumnHeader {
                            name: col.name().to_string(),
                            type_name: format!("{:?}", col.column_type()),
                        });
                    }
                }
            }
            QueryItem::Row(row) => {
                let mut row_values = Vec::new();
                for i in 0..row.columns().len() {
                    row_values.push(decode_mssql_cell(&row, i));
                }
                chunk.push(row_values);
                total_rows += 1;

                if chunk.len() >= chunk_size {
                    let payload = QueryResultPayload {
                        columns: columns.clone(),
                        rows: std::mem::take(&mut chunk),
                        execution_time_ms: start_time.elapsed().as_millis() as u64,
                        affected_rows: total_rows,
                    };
                    let event_name = format!("query_chunk_{}", query_id);
                    let _ = app_handle.emit(&event_name, payload);
                }
            }
        }
    }

    if !chunk.is_empty() {
        let payload = QueryResultPayload {
            columns: columns.clone(),
            rows: std::mem::take(&mut chunk),
            execution_time_ms: start_time.elapsed().as_millis() as u64,
            affected_rows: total_rows,
        };
        let event_name = format!("query_chunk_{}", query_id);
        let _ = app_handle.emit(&event_name, payload);
    }

    Ok(QueryResultPayload {
        columns,
        rows: Vec::new(),
        execution_time_ms: start_time.elapsed().as_millis() as u64,
        affected_rows: total_rows,
    })
}

fn decode_mssql_cell(row: &tiberius::Row, i: usize) -> Value {
    use tiberius::ColumnType;

    let col = &row.columns()[i];
    match col.column_type() {
        ColumnType::Null => Value::Null,
        ColumnType::Bit => {
            if let Ok(val) = row.try_get::<bool, _>(i) {
                json!(val)
            } else {
                Value::Null
            }
        }
        ColumnType::Int1 => {
            if let Ok(val) = row.try_get::<u8, _>(i) {
                json!(val)
            } else {
                Value::Null
            }
        }
        ColumnType::Int2 => {
            if let Ok(val) = row.try_get::<i16, _>(i) {
                json!(val)
            } else {
                Value::Null
            }
        }
        ColumnType::Int4 => {
            if let Ok(val) = row.try_get::<i32, _>(i) {
                json!(val)
            } else {
                Value::Null
            }
        }
        ColumnType::Int8 => {
            if let Ok(val) = row.try_get::<i64, _>(i) {
                json!(val)
            } else {
                Value::Null
            }
        }
        ColumnType::Float4 => {
            if let Ok(val) = row.try_get::<f32, _>(i) {
                json!(val)
            } else {
                Value::Null
            }
        }
        ColumnType::Float8 => {
            if let Ok(val) = row.try_get::<f64, _>(i) {
                json!(val)
            } else {
                Value::Null
            }
        }
        ColumnType::NVarchar | ColumnType::BigVarChar | ColumnType::NChar | ColumnType::BigChar => {
            if let Ok(val) = row.try_get::<&str, _>(i) {
                json!(val)
            } else {
                Value::Null
            }
        }
        _ => {
            if let Ok(Some(v)) = row.try_get::<&str, _>(i) {
                json!(v)
            } else {
                Value::Null
            }
        }
    }
}

pub fn split_redis_args(raw: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for ch in raw.chars() {
        match ch {
            '\'' | '"' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

fn format_redis_item(val: redis::Value) -> String {
    match val {
        redis::Value::Nil => "(nil)".to_string(),
        redis::Value::Int(i) => i.to_string(),
        redis::Value::BulkString(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        redis::Value::SimpleString(s) => s,
        redis::Value::Okay => "OK".to_string(),
        other => format!("{:?}", other),
    }
}

fn format_redis_value_to_payload(val: redis::Value, execution_time_ms: u64) -> Result<QueryResultPayload, String> {
    match val {
        redis::Value::Nil => Ok(QueryResultPayload {
            columns: vec![ColumnHeader { name: "Response".to_string(), type_name: "NIL".to_string() }],
            rows: vec![vec![json!(null)]],
            execution_time_ms,
            affected_rows: 0,
        }),
        redis::Value::Int(i) => Ok(QueryResultPayload {
            columns: vec![ColumnHeader { name: "Response".to_string(), type_name: "INTEGER".to_string() }],
            rows: vec![vec![json!(i)]],
            execution_time_ms,
            affected_rows: 1,
        }),
        redis::Value::BulkString(bytes) => {
            let s = String::from_utf8_lossy(&bytes).to_string();
            Ok(QueryResultPayload {
                columns: vec![ColumnHeader { name: "Response".to_string(), type_name: "BULK_STRING".to_string() }],
                rows: vec![vec![json!(s)]],
                execution_time_ms,
                affected_rows: 1,
            })
        }
        redis::Value::SimpleString(s) => Ok(QueryResultPayload {
            columns: vec![ColumnHeader { name: "Response".to_string(), type_name: "SIMPLE_STRING".to_string() }],
            rows: vec![vec![json!(s)]],
            execution_time_ms,
            affected_rows: 1,
        }),
        redis::Value::Array(items) => {
            let mut rows = Vec::new();
            for (idx, item) in items.into_iter().enumerate() {
                let formatted = format_redis_item(item);
                rows.push(vec![json!(idx), json!(formatted)]);
            }
            let len = rows.len() as u64;
            Ok(QueryResultPayload {
                columns: vec![
                    ColumnHeader { name: "Index".to_string(), type_name: "INTEGER".to_string() },
                    ColumnHeader { name: "Value".to_string(), type_name: "STRING".to_string() },
                ],
                rows,
                execution_time_ms,
                affected_rows: len,
            })
        }
        redis::Value::Map(pairs) => {
            let mut rows = Vec::new();
            for (k, v) in pairs {
                rows.push(vec![json!(format_redis_item(k)), json!(format_redis_item(v))]);
            }
            let len = rows.len() as u64;
            Ok(QueryResultPayload {
                columns: vec![
                    ColumnHeader { name: "Key / Field".to_string(), type_name: "STRING".to_string() },
                    ColumnHeader { name: "Value".to_string(), type_name: "STRING".to_string() },
                ],
                rows,
                execution_time_ms,
                affected_rows: len,
            })
        }
        redis::Value::Okay => Ok(QueryResultPayload {
            columns: vec![ColumnHeader { name: "Response".to_string(), type_name: "OK".to_string() }],
            rows: vec![vec![json!("OK")]],
            execution_time_ms,
            affected_rows: 1,
        }),
        _ => Ok(QueryResultPayload {
            columns: vec![ColumnHeader { name: "Response".to_string(), type_name: "UNKNOWN".to_string() }],
            rows: vec![vec![json!(format!("{:?}", val))]],
            execution_time_ms,
            affected_rows: 1,
        }),
    }
}

pub async fn execute_mongo_query(
    managed_conn: &ManagedConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let client = managed_conn
        .mongo_client
        .as_ref()
        .ok_or_else(|| "MongoDB client is not active or connected.".to_string())?;

    let db_name = client
        .default_database()
        .map(|db| db.name().to_string())
        .unwrap_or_else(|| "test".to_string());

    let trimmed = sql.trim();
    let json_val: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("Invalid MongoDB JSON command format: {}", e))?;

    let bson_doc = mongodb::bson::to_document(&json_val)
        .map_err(|e| format!("Failed to convert JSON to BSON Document: {}", e))?;

    let start = Instant::now();
    let res_doc = client
        .database(&db_name)
        .run_command(bson_doc)
        .await
        .map_err(|e| format!("MongoDB command execution failed: {}", e))?;

    let elapsed = start.elapsed().as_millis() as u64;

    if let Ok(cursor) = res_doc.get_document("cursor") {
        if let Ok(first_batch) = cursor.get_array("firstBatch") {
            let mut rows = Vec::new();
            for item in first_batch {
                if let Some(doc) = item.as_document() {
                    let json_v: serde_json::Value = mongodb::bson::from_document(doc.clone()).unwrap_or(json!(null));
                    rows.push(vec![json_v]);
                }
            }
            let len = rows.len() as u64;
            return Ok(QueryResultPayload {
                columns: vec![ColumnHeader {
                    name: "Document".to_string(),
                    type_name: "BSON".to_string(),
                }],
                rows,
                execution_time_ms: elapsed,
                affected_rows: len,
            });
        }
    }

    let json_res: serde_json::Value = mongodb::bson::from_document(res_doc).unwrap_or(json!(null));
    Ok(QueryResultPayload {
        columns: vec![ColumnHeader {
            name: "Result".to_string(),
            type_name: "BSON".to_string(),
        }],
        rows: vec![vec![json_res]],
        execution_time_ms: elapsed,
        affected_rows: 1,
    })
}

pub async fn execute_redis_query(
    managed_conn: &ManagedConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let mut conn = managed_conn
        .redis_client
        .clone()
        .ok_or_else(|| "Redis connection is not active or unavailable.".to_string())?;

    let args = split_redis_args(sql.trim());
    if args.is_empty() {
        return Err("Empty Redis command provided.".to_string());
    }

    let start = Instant::now();
    let mut cmd = redis::cmd(&args[0]);
    for arg in &args[1..] {
        cmd.arg(arg);
    }

    let val: redis::Value = cmd
        .query_async(&mut conn)
        .await
        .map_err(|e| format!("Redis command failed: {}", e))?;

    let elapsed = start.elapsed().as_millis() as u64;
    format_redis_value_to_payload(val, elapsed)
}

pub async fn execute_scylla_query(
    managed_conn: &ManagedConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let session = managed_conn
        .scylla_session
        .as_ref()
        .ok_or_else(|| "Cassandra / ScyllaDB session is not active or connected.".to_string())?;

    let start = Instant::now();
    let query_result = session
        .query_unpaged(sql, ())
        .await
        .map_err(|e| format!("Scylla/CQL query failed: {}", e))?;

    let elapsed = start.elapsed().as_millis() as u64;

    let columns: Vec<ColumnHeader> = query_result
        .col_specs()
        .iter()
        .map(|col| ColumnHeader {
            name: col.name.clone(),
            type_name: format!("{:?}", col.typ),
        })
        .collect();

    let mut rows = Vec::new();
    if let Ok(cql_rows) = query_result.rows() {
        for row in cql_rows {
            let mut row_cells = Vec::new();
            for col in &row.columns {
                let json_val = match col {
                    Some(val) => json!(format!("{:?}", val)),
                    None => json!(null),
                };
                row_cells.push(json_val);
            }
            rows.push(row_cells);
        }
    }

    let len = rows.len() as u64;
    Ok(QueryResultPayload {
        columns,
        rows,
        execution_time_ms: elapsed,
        affected_rows: len,
    })
}

pub async fn execute_clickhouse_query(
    managed_conn: &ManagedConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let client = managed_conn
        .clickhouse_client
        .as_ref()
        .ok_or_else(|| "ClickHouse client is not active or connected.".to_string())?;

    let start = Instant::now();
    let mut cursor = client
        .query(sql)
        .fetch::<String>()
        .map_err(|e| format!("ClickHouse query failed: {}", e))?;

    let mut rows = Vec::new();
    while let Some(row) = cursor
        .next()
        .await
        .map_err(|e| format!("Failed reading ClickHouse row stream: {}", e))?
    {
        rows.push(vec![json!(row)]);
    }

    let elapsed = start.elapsed().as_millis() as u64;
    let len = rows.len() as u64;

    Ok(QueryResultPayload {
        columns: vec![ColumnHeader {
            name: "Result".to_string(),
            type_name: "STRING".to_string(),
        }],
        rows,
        execution_time_ms: elapsed,
        affected_rows: len,
    })
}

pub async fn execute_duckdb_query(
    _managed_conn: &ManagedConnection,
    _sql: &str,
) -> Result<QueryResultPayload, String> {
    Err("DuckDB native execution is stubbed but not yet fully implemented.".to_string())
}

pub async fn execute_libsql_query(
    managed_conn: &ManagedConnection,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let cfg = managed_conn
        .turso_config
        .as_ref()
        .ok_or_else(|| "Turso connection configuration is missing.".to_string())?;
    crate::db::turso_engine::run_turso_query(cfg, sql)
}

pub async fn execute_snowflake_query(
    _managed_conn: &ManagedConnection,
    _sql: &str,
) -> Result<QueryResultPayload, String> {
    Err("Snowflake native execution is stubbed but not yet fully implemented.".to_string())
}

pub async fn execute_oracle_query(
    _managed_conn: &ManagedConnection,
    _sql: &str,
) -> Result<QueryResultPayload, String> {
    Err("Oracle native execution is stubbed but not yet fully implemented.".to_string())
}
