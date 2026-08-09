// Turso (libsql over HTTP v2 pipeline API) engine driver module
use crate::db::executor::{ColumnHeader, QueryResultPayload};
use crate::db::introspection::{ColumnInfo, TableInfo};
use crate::db::pool::{ConnectionDetails, TestConnectionResult};
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TursoConfig {
    pub url: String,
    pub auth_token: String,
}

impl TursoConfig {
    pub fn from_details(details: &ConnectionDetails) -> Result<Self, String> {
        let raw_host = if !details.host.is_empty() {
            &details.host
        } else {
            &details.database
        };

        if raw_host.is_empty() {
            return Err("Turso connection requires a database host URL (e.g., libsql://test-sasuke.aws-ap-south-1.turso.io)".to_string());
        }

        let mut url_str = raw_host.trim().to_string();
        let mut extracted_token = String::new();

        // Support token embedded in URL query parameters: ?authToken=... or &authToken=... or ?jwt=...
        if let Some(pos) = url_str.find("authToken=") {
            let after = &url_str[pos + "authToken=".len()..];
            let token_part = after.split('&').next().unwrap_or(after);
            extracted_token = token_part.to_string();
            if let Some(q_pos) = url_str.find('?') {
                url_str.truncate(q_pos);
            }
        } else if let Some(pos) = url_str.find("jwt=") {
            let after = &url_str[pos + "jwt=".len()..];
            let token_part = after.split('&').next().unwrap_or(after);
            extracted_token = token_part.to_string();
            if let Some(q_pos) = url_str.find('?') {
                url_str.truncate(q_pos);
            }
        }

        let mut url = url_str;
        if url.starts_with("libsql://") {
            url = url.replace("libsql://", "https://");
        } else if !url.starts_with("http://") && !url.starts_with("https://") {
            url = format!("https://{}", url);
        }
        url = url.trim_end_matches('/').to_string();

        let auth_token = if let Some(ref p) = details.password {
            let trimmed = p.trim();
            if !trimmed.is_empty() {
                trimmed.to_string()
            } else {
                extracted_token
            }
        } else {
            extracted_token
        };

        Ok(Self { url, auth_token })
    }

    pub fn pipeline_url(&self) -> String {
        format!("{}/v2/pipeline", self.url)
    }
}

#[derive(Serialize)]
struct TursoStmt {
    sql: String,
}

#[derive(Serialize)]
struct TursoRequest {
    #[serde(rename = "type")]
    req_type: String,
    stmt: TursoStmt,
}

#[derive(Serialize)]
struct TursoPipelinePayload {
    requests: Vec<TursoRequest>,
}

#[derive(Deserialize, Debug)]
struct TursoCol {
    name: String,
    decltype: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum TursoValue {
    Typed {
        #[serde(rename = "type")]
        val_type: String,
        value: Option<serde_json::Value>,
        base64: Option<String>,
    },
    Raw(serde_json::Value),
}

impl TursoValue {
    fn to_json_value(&self) -> serde_json::Value {
        match self {
            TursoValue::Typed { val_type, value, base64 } => {
                match val_type.as_str() {
                    "null" => serde_json::Value::Null,
                    "integer" => {
                        if let Some(v) = value {
                            if let Some(s) = v.as_str() {
                                if let Ok(n) = s.parse::<i64>() {
                                    return serde_json::Value::Number(n.into());
                                }
                            }
                            v.clone()
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    "float" => value.clone().unwrap_or(serde_json::Value::Null),
                    "text" => value.clone().unwrap_or(serde_json::Value::Null),
                    "blob" => {
                        if let Some(b) = base64 {
                            serde_json::Value::String(format!("BLOB({})", b))
                        } else {
                            serde_json::Value::Null
                        }
                    }
                    _ => value.clone().unwrap_or(serde_json::Value::Null),
                }
            }
            TursoValue::Raw(val) => val.clone(),
        }
    }
}

#[derive(Deserialize, Debug)]
struct TursoExecResult {
    cols: Option<Vec<TursoCol>>,
    rows: Option<Vec<Vec<TursoValue>>>,
    affected_row_count: Option<u64>,
}

#[derive(Deserialize, Debug)]
struct TursoResponseItem {
    #[serde(rename = "type")]
    resp_type: String,
    result: Option<TursoExecResult>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct TursoResultItem {
    #[serde(rename = "type")]
    res_type: String,
    response: Option<TursoResponseItem>,
    error: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct TursoPipelineResponse {
    results: Option<Vec<TursoResultItem>>,
    error: Option<serde_json::Value>,
}

/// Execute a SQL statement against Turso HTTP v2 pipeline API
pub fn execute_turso_sql(
    config: &TursoConfig,
    sql: &str,
) -> Result<(Vec<ColumnHeader>, Vec<Vec<serde_json::Value>>, u64), String> {
    let payload = TursoPipelinePayload {
        requests: vec![
            TursoRequest {
                req_type: "execute".to_string(),
                stmt: TursoStmt {
                    sql: sql.to_string(),
                },
            },
            TursoRequest {
                req_type: "close".to_string(),
                stmt: TursoStmt {
                    sql: "".to_string(),
                },
            },
        ],
    };

    let mut req = ureq::post(&config.pipeline_url())
        .set("Content-Type", "application/json");

    if !config.auth_token.is_empty() {
        req = req.set("Authorization", &format!("Bearer {}", config.auth_token));
    }

    let resp_str = req
        .send_json(&payload)
        .map_err(|e| format!("Turso HTTP request failed: {}", e))?
        .into_string()
        .map_err(|e| format!("Failed to read Turso response: {}", e))?;

    let parsed: TursoPipelineResponse = serde_json::from_str(&resp_str)
        .map_err(|e| format!("Failed to parse Turso JSON response: {} (raw: {})", e, resp_str))?;

    if let Some(err) = parsed.error {
        return Err(format!("Turso server error: {}", err));
    }

    let results = parsed
        .results
        .ok_or_else(|| "Turso pipeline returned no results".to_string())?;

    if results.is_empty() {
        return Err("Turso returned empty result set".to_string());
    }

    let first = &results[0];
    if first.res_type == "error" {
        return Err(format!(
            "Turso query error: {}",
            first.error.as_ref().map(|e| e.to_string()).unwrap_or_default()
        ));
    }

    let resp = first
        .response
        .as_ref()
        .ok_or_else(|| "Turso response item missing".to_string())?;

    if resp.resp_type == "error" {
        return Err(format!(
            "Turso execution error: {}",
            resp.error.as_ref().map(|e| e.to_string()).unwrap_or_default()
        ));
    }

    let exec_res = resp
        .result
        .as_ref()
        .ok_or_else(|| "Turso exec result missing".to_string())?;

    let columns: Vec<ColumnHeader> = exec_res
        .cols
        .as_ref()
        .map(|cols| {
            cols.iter()
                .map(|c| ColumnHeader {
                    name: c.name.clone(),
                    type_name: c.decltype.clone().unwrap_or_else(|| "TEXT".to_string()),
                })
                .collect()
        })
        .unwrap_or_default();

    let affected = exec_res.affected_row_count.unwrap_or(0);

    let mut rows_matrix: Vec<Vec<serde_json::Value>> = Vec::new();
    if let Some(raw_rows) = &exec_res.rows {
        for row in raw_rows {
            let mut row_cells = Vec::new();
            for (idx, _col) in columns.iter().enumerate() {
                let val = if idx < row.len() {
                    row[idx].to_json_value()
                } else {
                    serde_json::Value::Null
                };
                row_cells.push(val);
            }
            rows_matrix.push(row_cells);
        }
    }

    Ok((columns, rows_matrix, affected))
}

pub fn test_turso_connection(details: &ConnectionDetails) -> TestConnectionResult {
    let start = Instant::now();
    let config = match TursoConfig::from_details(details) {
        Ok(c) => c,
        Err(e) => {
            return TestConnectionResult {
                success: false,
                latency_ms: 0,
                message: e,
            }
        }
    };

    match execute_turso_sql(&config, "SELECT 1 as ping;") {
        Ok(_) => TestConnectionResult {
            success: true,
            latency_ms: start.elapsed().as_millis() as u64,
            message: format!("Successfully connected to Turso database at {}", config.url),
        },
        Err(e) => TestConnectionResult {
            success: false,
            latency_ms: start.elapsed().as_millis() as u64,
            message: e,
        },
    }
}

pub fn run_turso_query(
    config: &TursoConfig,
    sql: &str,
) -> Result<QueryResultPayload, String> {
    let start = Instant::now();
    let (columns, rows, affected) = execute_turso_sql(config, sql)?;
    let latency = start.elapsed().as_millis() as u64;

    Ok(QueryResultPayload {
        columns,
        rows,
        execution_time_ms: latency,
        affected_rows: affected,
    })
}

pub fn fetch_turso_tables(config: &TursoConfig) -> Result<Vec<TableInfo>, String> {
    let sql = "SELECT name, type FROM sqlite_master WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name;";
    let (cols, rows, _) = execute_turso_sql(config, sql)?;

    let name_idx = cols.iter().position(|c| c.name.to_lowercase() == "name").unwrap_or(0);
    let type_idx = cols.iter().position(|c| c.name.to_lowercase() == "type").unwrap_or(1);

    let mut tables = Vec::new();
    for r in rows {
        let name = r.get(name_idx).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let t_type = r.get(type_idx).and_then(|v| v.as_str()).unwrap_or("table");
        if !name.is_empty() {
            let info = TableInfo::new("main", &name, if t_type == "view" { "VIEW" } else { "BASE TABLE" });
            tables.push(info);
        }
    }
    Ok(tables)
}

pub fn fetch_turso_columns(
    config: &TursoConfig,
    table_name: &str,
) -> Result<Vec<ColumnInfo>, String> {
    let clean_table = table_name.replace('"', "\"\"");
    let sql = format!("PRAGMA table_info(\"{}\");", clean_table);
    let (cols, rows, _) = execute_turso_sql(config, &sql)?;

    let name_idx = cols.iter().position(|c| c.name.to_lowercase() == "name").unwrap_or(1);
    let type_idx = cols.iter().position(|c| c.name.to_lowercase() == "type").unwrap_or(2);
    let notnull_idx = cols.iter().position(|c| c.name.to_lowercase() == "notnull").unwrap_or(3);
    let pk_idx = cols.iter().position(|c| c.name.to_lowercase() == "pk").unwrap_or(5);

    let mut columns = Vec::new();
    for r in rows {
        let name = r.get(name_idx).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let data_type = r.get(type_idx).and_then(|v| v.as_str()).unwrap_or("TEXT").to_string();
        let notnull = r.get(notnull_idx).and_then(|v| v.as_i64()).unwrap_or(0);
        let pk = r.get(pk_idx).and_then(|v| v.as_i64()).unwrap_or(0);

        if !name.is_empty() {
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
    Ok(columns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turso_config_parsing() {
        let details = ConnectionDetails {
            db_type: "turso".to_string(),
            host: "libsql://my-db-org.turso.io".to_string(),
            port: 0,
            user: "".to_string(),
            password: Some("test_jwt_token".to_string()),
            database: "".to_string(),
            ssl_mode: None,
            cloud_iam: None,
            is_read_only: false,
        };

        let cfg = TursoConfig::from_details(&details).unwrap();
        assert_eq!(cfg.url, "https://my-db-org.turso.io");
        assert_eq!(cfg.auth_token, "test_jwt_token");
        assert_eq!(cfg.pipeline_url(), "https://my-db-org.turso.io/v2/pipeline");
    }
}
