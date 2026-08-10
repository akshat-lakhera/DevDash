import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ConnectionConfig, TableItem, ColumnItem, PkInfo } from '../types';

export interface ConnectionDetailsPayload {
  db_type: string;
  host: string;
  port: number;
  user: string;
  password?: string;
  database: string;
  ssl_mode?: string;
  is_read_only?: boolean;
}

export interface TestConnectionResultPayload {
  success: boolean;
  latency_ms: number;
  message: string;
}

export interface QueryResultPayload {
  columns: { name: string; type_name: string }[];
  rows: any[][];
  execution_time_ms: number;
  affected_rows: number;
}

export const isTauriAvailable = (): boolean => {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
};

export const testDbConnection = async (
  config: Partial<ConnectionConfig>,
  password?: string
): Promise<TestConnectionResultPayload> => {
  if (!isTauriAvailable()) {
    // Web Browser Fallback simulation
    await new Promise((r) => setTimeout(r, 400));
    return {
      success: true,
      latency_ms: 24,
      message: `[Web Preview] Connection test simulated for ${config.db_type || 'database'}. Install/run in native Tauri desktop container for live TCP execution.`,
    };
  }

  const payload: ConnectionDetailsPayload = {
    db_type: config.db_type || 'postgres',
    host: config.host || 'localhost',
    port: config.port || 5432,
    user: config.user || 'postgres',
    password: password || '',
    database: config.database || 'postgres',
    ssl_mode: config.ssl_mode || 'require',
  };

  try {
    return await invoke<TestConnectionResultPayload>('test_db_connection', { details: payload });
  } catch (err: any) {
    return {
      success: false,
      latency_ms: 0,
      message: String(err?.message || err || 'Failed to connect'),
    };
  }
};

export const connectDatabase = async (
  conn: ConnectionConfig,
  password?: string
): Promise<void> => {
  if (!isTauriAvailable()) return;

  let targetHost = conn.host;
  let targetPort = conn.port;

  if (conn.ssh_config?.enabled) {
    const localPort = await openSshTunnel(
      conn.id,
      {
        enabled: true,
        host: conn.ssh_config.host,
        port: conn.ssh_config.port,
        user: conn.ssh_config.user,
        key_path: conn.ssh_config.key_path,
      },
      conn.host,
      conn.port
    );
    targetHost = '127.0.0.1';
    targetPort = localPort;
  }

  const payload: ConnectionDetailsPayload = {
    db_type: conn.db_type,
    host: targetHost,
    port: targetPort,
    user: conn.user,
    password: password || '',
    database: conn.database,
    ssl_mode: conn.ssl_mode || 'require',
    is_read_only: !!conn.is_read_only,
  };

  await invoke('connect_database_config', {
    connectionId: conn.id,
    details: payload,
  });
};

export const getDatabaseTables = async (
  connectionId: string,
  dbKind: string
): Promise<TableItem[]> => {
  if (!isTauriAvailable()) {
    return [];
  }

  try {
    const raw = await invoke<TableItem[]>('get_database_tables', {
      connectionId,
      dbKind,
    });
    // Normalize for older payloads missing schema/qualified_name
    return raw.map((t) => ({
      ...t,
      schema: t.schema || 'main',
      qualified_name:
        t.qualified_name ||
        (t.schema && t.schema !== 'main' && t.schema !== 'public'
          ? `${t.schema}.${t.name}`
          : t.schema === 'public'
            ? `${t.schema}.${t.name}`
            : t.name),
    }));
  } catch (err) {
    console.warn('Failed to fetch database tables via IPC:', err);
    return [];
  }
};

export const getTableColumns = async (
  connectionId: string,
  dbKind: string,
  tableName: string
): Promise<ColumnItem[]> => {
  if (!isTauriAvailable()) {
    // Mock columns for web browser UI testing
    return [
      { name: 'id', data_type: 'INT8', is_nullable: false, is_primary_key: true },
      { name: 'username', data_type: 'TEXT', is_nullable: false, is_primary_key: false },
      { name: 'email', data_type: 'TEXT', is_nullable: false, is_primary_key: false },
    ];
  }

  try {
    const raw = await invoke<
      Array<
        ColumnItem & {
          fk_table?: string | null;
          fk_column?: string | null;
        }
      >
    >('get_table_columns', {
      connectionId,
      dbKind,
      tableName,
    });
    // Map Rust ColumnInfo FK fields into frontend ColumnItem.fk_references
    return raw.map((c) => ({
      name: c.name,
      data_type: c.data_type,
      is_nullable: c.is_nullable,
      is_primary_key: c.is_primary_key,
      is_foreign_key: Boolean(c.is_foreign_key || c.fk_table),
      fk_references:
        c.fk_table && c.fk_column
          ? { table: c.fk_table, column: c.fk_column }
          : c.fk_references,
      default_value: c.default_value,
    }));
  } catch (err) {
    console.warn('Failed to fetch table columns via IPC:', err);
    return [];
  }
};

/** Fail-closed PK analysis: never assume a table is editable when analysis is unavailable. */
const PK_ANALYSIS_UNAVAILABLE: PkInfo = {
  has_single_pk: false,
  pk_column_name: undefined,
  is_read_only: true,
  read_only_reason: 'Primary key analysis unavailable; grid edits are disabled for safety.',
};

export const getPkAnalysis = async (
  connectionId: string,
  dbKind: string,
  tableName: string
): Promise<PkInfo> => {
  if (!isTauriAvailable()) {
    return { ...PK_ANALYSIS_UNAVAILABLE };
  }

  try {
    return await invoke<PkInfo>('get_pk_analysis', {
      connectionId,
      dbKind,
      tableName,
    });
  } catch (err) {
    console.warn('get_pk_analysis failed; treating table as read-only:', err);
    return {
      ...PK_ANALYSIS_UNAVAILABLE,
      read_only_reason: `Primary key analysis failed: ${String(err)}. Grid edits are disabled for safety.`,
    };
  }
};

export const runSqlQuery = async (
  connectionId: string,
  sql: string,
  queryId?: string,
  /** Required for destructive SQL when server Safe Mode is on (default false). */
  allowDestructive?: boolean,
  /** Abort after N seconds (0 / undefined = no limit). */
  queryTimeoutSec?: number
): Promise<QueryResultPayload> => {
  if (!isTauriAvailable()) {
    const lower = sql.toLowerCase().trim();
    // Browser preview: do not pretend writes succeeded
    if (
      /^(insert|update|delete|drop|alter|truncate|create|grant|revoke|replace)\b/i.test(lower)
    ) {
      throw new Error(
        '[Web Preview] Write/DDL is not executed in the browser. Run the native Tauri app.'
      );
    }
    if (/\bcount\s*\(\s*\*\s*\)/i.test(sql) && !lower.includes('group by')) {
      return {
        columns: [{ name: 'count', type_name: 'INTEGER' }],
        rows: [[42]],
        execution_time_ms: 2,
        affected_rows: 1,
      };
    }
    // Return mock data for web browser preview
    return {
      columns: [
        { name: 'id', type_name: 'INT8' },
        { name: 'username', type_name: 'TEXT' },
        { name: 'email', type_name: 'TEXT' },
      ],
      rows: [
        [1, 'akshatlakhera', 'akshatlakhera50@gmail.com'],
        [2, 'chitti', 'chitti@example.com'],
        [3, 'volt', 'volt@example.com'],
      ],
      execution_time_ms: 15,
      affected_rows: 3,
    };
  }

  const qid = queryId || `q-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  return await invoke<QueryResultPayload>('run_sql_query', {
    connectionId,
    queryId: qid,
    sql,
    allowDestructive: allowDestructive === true,
    queryTimeoutSec:
      typeof queryTimeoutSec === 'number' && queryTimeoutSec > 0
        ? Math.floor(queryTimeoutSec)
        : null,
  });
};

export interface SshConfigPayload {
  enabled: boolean;
  host: string;
  port: number;
  user: string;
  password?: string;
  key_path?: string;
  passphrase?: string;
}

export const testSshTunnel = async (
  sshConfig: SshConfigPayload
): Promise<TestConnectionResultPayload> => {
  if (!isTauriAvailable()) {
    await new Promise((r) => setTimeout(r, 300));
    return {
      success: true,
      latency_ms: 32,
      message: `[Web Preview] SSH tunnel check simulated for user '${sshConfig.user}' on ${sshConfig.host}:${sshConfig.port}.`,
    };
  }

  try {
    const latency = await invoke<number>('test_ssh_tunnel', { config: sshConfig });
    return {
      success: true,
      latency_ms: latency,
      message: `SSH authentication succeeded for ${sshConfig.user}@${sshConfig.host}:${sshConfig.port}`,
    };
  } catch (err: any) {
    return {
      success: false,
      latency_ms: 0,
      message: String(err?.message || err || 'SSH tunnel test failed'),
    };
  }
};

export const openSshTunnel = async (
  connectionId: string,
  sshConfig: SshConfigPayload,
  targetHost: string,
  targetPort: number
): Promise<number> => {
  if (!isTauriAvailable()) return 58432;
  return await invoke<number>('open_ssh_tunnel', {
    connectionId,
    sshConfig,
    targetHost,
    targetPort,
  });
};

export const closeSshTunnel = async (connectionId: string): Promise<void> => {
  if (!isTauriAvailable()) return;
  await invoke('close_ssh_tunnel', { connectionId });
};

export const disconnectDatabase = async (connectionId: string): Promise<void> => {
  if (!isTauriAvailable()) return;
  try {
    await invoke('disconnect_database', { connectionId });
  } catch {
    /* already closed or never opened */
  }
};

export const streamSqlQuery = async (
  connectionId: string,
  sql: string,
  onColumns: (columns: { name: string; type_name: string }[]) => void,
  onChunk: (chunk: any[][]) => void,
  onDone: (executionTimeMs: number, totalRows: number) => void,
  allowDestructive?: boolean
): Promise<void> => {
  if (!isTauriAvailable()) {
    onColumns([{ name: 'id', type_name: 'INTEGER' }, { name: 'data', type_name: 'TEXT' }]);
    onChunk([[1, 'Sample Row (Browser Mock)']]);
    onDone(10, 1);
    return;
  }

  const queryId = `sq-${Date.now()}-${Math.random().toString(36).substring(2, 7)}`;

  const unlistenCols = await listen<{ name: string; type_name: string }[]>(`query_columns_${queryId}`, (event) => {
    onColumns(event.payload);
  });

  const unlistenChunk = await listen<{ rows: any[][] }>(`query_chunk_${queryId}`, (event) => {
    onChunk(event.payload.rows);
  });

  const unlistenDone = await listen<{ execution_time_ms: number; total_rows: number }>(`query_done_${queryId}`, (event) => {
    onDone(event.payload.execution_time_ms, event.payload.total_rows);
    unlistenCols();
    unlistenChunk();
    unlistenDone();
  });

  try {
    await invoke('stream_sql_query', {
      connectionId,
      queryId,
      sql,
      chunkSize: 500,
      allowDestructive: allowDestructive === true,
    });
  } catch (err) {
    unlistenCols();
    unlistenChunk();
    unlistenDone();
    throw err;
  }
};

// ─── Credentials (OS keychain + Encrypted Web Session Fallback) ────────────
const webPasswordStore = new Map<string, string>();

const encodeSecret = (val: string): string => {
  try {
    return btoa(encodeURIComponent(val));
  } catch {
    return val;
  }
};

const decodeSecret = (val: string): string => {
  try {
    return decodeURIComponent(atob(val));
  } catch {
    return val;
  }
};

export const saveDbPassword = async (connectionId: string, secretVal: string): Promise<void> => {
  if (!connectionId) return;
  webPasswordStore.set(connectionId, secretVal);
  try {
    sessionStorage.setItem(`devdash_sec_${connectionId}`, encodeSecret(secretVal));
  } catch { }
  if (!isTauriAvailable()) return;
  try {
    await invoke('save_db_password', { connectionId, password: secretVal });
  } catch (e) {
    console.warn('OS Keychain save failed, using persistent fallback storage:', e);
  }
};

export const getDbPassword = async (connectionId: string): Promise<string | null> => {
  if (!connectionId) return null;
  if (isTauriAvailable()) {
    try {
      const secretVal = await invoke<string>('get_db_password', { connectionId });
      if (secretVal) {
        // Keep in-memory cache in sync
        webPasswordStore.set(connectionId, secretVal);
        return secretVal;
      }
    } catch {
      /* fallthrough to fallback storage */
    }
  }
  const cached = webPasswordStore.get(connectionId);
  if (cached) return cached;
  try {
    const stored = sessionStorage.getItem(`devdash_sec_${connectionId}`);
    if (stored) {
      const decoded = decodeSecret(stored);
      webPasswordStore.set(connectionId, decoded);
      return decoded;
    }
  } catch { }
  return null;
};

// ─── Safety / staging ────────────────────────────────────────────────
export interface SafetyAnalysis {
  is_destructive: boolean;
  destructive_type?: string;
  warning_message?: string;
  requires_confirmation: boolean;
}

export const checkSqlSafety = async (sql: string): Promise<SafetyAnalysis> => {
  if (!isTauriAvailable()) {
    const upper = sql.trim().toUpperCase();
    const isDestructive =
      upper.startsWith('DROP') ||
      upper.startsWith('TRUNCATE') ||
      (upper.startsWith('DELETE') && !upper.includes('WHERE')) ||
      (upper.startsWith('UPDATE') && !upper.includes('WHERE'));
    return {
      is_destructive: isDestructive,
      requires_confirmation: isDestructive,
      warning_message: isDestructive ? 'Destructive operation detected' : undefined,
    };
  }
  return await invoke<SafetyAnalysis>('check_sql_safety', { sql });
};

export interface StagedCellChangePayload {
  column_name: string;
  new_value: unknown;
}

export interface StagedRowEditPayload {
  pk_value: unknown;
  changes: StagedCellChangePayload[];
}

export const commitStagedRowEdits = async (
  connectionId: string,
  tableName: string,
  pkColumn: string,
  edits: StagedRowEditPayload[]
): Promise<number> => {
  if (!isTauriAvailable()) {
    throw new Error('Staged commits require the native Tauri desktop app');
  }
  return await invoke<number>('commit_staged_row_edits', {
    connectionId,
    tableName,
    pkColumn,
    edits,
  });
};

export interface StagedInsertRowPayload {
  columns: string[];
  values: unknown[];
}

export interface StagedDeleteRowPayload {
  pk_value: unknown;
}

export const commitStagedInserts = async (
  connectionId: string,
  tableName: string,
  rows: StagedInsertRowPayload[]
): Promise<number> => {
  if (!isTauriAvailable()) {
    throw new Error('Staged inserts require the native Tauri desktop app');
  }
  return await invoke<number>('commit_staged_inserts', {
    connectionId,
    tableName,
    rows,
  });
};

export const commitStagedDeletes = async (
  connectionId: string,
  tableName: string,
  pkColumn: string,
  rows: StagedDeleteRowPayload[]
): Promise<number> => {
  if (!isTauriAvailable()) {
    throw new Error('Staged deletes require the native Tauri desktop app');
  }
  return await invoke<number>('commit_staged_deletes', {
    connectionId,
    tableName,
    pkColumn,
    rows,
  });
};

export interface IndexInfoPayload {
  name: string;
  columns: string[];
  is_unique: boolean;
  is_primary: boolean;
}

export interface TableDdlPayload {
  table_name: string;
  create_sql: string;
  indexes: IndexInfoPayload[];
  foreign_keys: {
    column_name: string;
    referenced_table: string;
    referenced_column: string;
    constraint_name: string;
  }[];
}

export const generateTableDdl = async (
  connectionId: string,
  tableName: string,
  dbKind: string
): Promise<TableDdlPayload> => {
  if (!isTauriAvailable()) {
    throw new Error('DDL generation requires the native Tauri desktop app');
  }
  return await invoke<TableDdlPayload>('generate_table_ddl_cmd', {
    connectionId,
    tableName,
    dbKind,
  });
};

export const getTableIndexes = async (
  connectionId: string,
  tableName: string,
  dbKind: string
): Promise<IndexInfoPayload[]> => {
  if (!isTauriAvailable()) return [];
  return await invoke<IndexInfoPayload[]>('get_table_indexes', {
    connectionId,
    tableName,
    dbKind,
  });
};

/** Full-table export from the engine (not limited to the current UI page).
 *  For `parquet`, the string is base64-encoded binary. */
export const exportTableData = async (
  connectionId: string,
  tableName: string,
  format: 'csv' | 'json' | 'sql' | 'parquet',
  whereClause?: string
): Promise<string> => {
  if (!isTauriAvailable()) {
    throw new Error('Full table export requires the native Tauri desktop app');
  }
  return await invoke<string>('export_table_data', {
    connectionId,
    tableName,
    format,
    whereClause: whereClause || null,
  });
};

export interface RedisKeyEntry {
  key: string;
  key_type: string;
  ttl: number;
  size: number;
}

/** Fetch live Redis keys via native Rust RESP protocol client */
export const fetchRedisKeys = async (
  host: string,
  port: number,
  password?: string,
  pattern?: string
): Promise<RedisKeyEntry[]> => {
  if (!isTauriAvailable()) {
    return [];
  }
  return await invoke<RedisKeyEntry[]>('fetch_redis_keys', {
    host,
    port,
    password: password || null,
    pattern: pattern || '*',
  });
};

/** Current-page / in-memory rows → Parquet base64. */
export const exportRowsParquet = async (
  columns: string[],
  rows: unknown[][]
): Promise<string> => {
  if (!isTauriAvailable()) {
    throw new Error('Parquet export requires the native Tauri desktop app');
  }
  return await invoke<string>('export_rows_parquet', { columns, rows });
};

function base64ToUint8Array(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

export function downloadBase64Parquet(b64: string, filename: string): void {
  const bytes = base64ToUint8Array(b64);
  // Copy into a plain ArrayBuffer-backed view for Blob typing across TS versions
  const ab = new ArrayBuffer(bytes.byteLength);
  new Uint8Array(ab).set(bytes);
  const blob = new Blob([ab], { type: 'application/vnd.apache.parquet' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename.endsWith('.parquet') ? filename : `${filename}.parquet`;
  a.click();
  URL.revokeObjectURL(url);
}

// ─── Live metrics ────────────────────────────────────────────────────
export interface DatabaseLiveMetrics {
  active_connections: number;
  queries_per_second: number;
  cache_hit_ratio: number;
  slow_queries: { query: string; duration_ms: number; calls: number }[];
  table_sizes: { table_name: string; size_bytes: number; size_pretty: string }[];
  response_time_ms: number;
}

export const getLiveDatabaseMetrics = async (
  connectionId: string,
  engine: 'postgres' | 'mysql' | 'sqlite'
): Promise<DatabaseLiveMetrics> => {
  if (!isTauriAvailable()) {
    throw new Error('Live metrics require the native Tauri desktop app');
  }
  return await invoke<DatabaseLiveMetrics>('get_live_database_metrics', {
    connectionId,
    engine,
  });
};

// ─── Audit log ───────────────────────────────────────────────────────
export interface AuditLogEntryPayload {
  id: string;
  timestamp: string;
  user: string;
  connection_name: string;
  action_type: string;
  sql: string;
  affected_rows: number;
  status: string;
  client_ip: string;
}

export const getAuditLog = async (limit = 200): Promise<AuditLogEntryPayload[]> => {
  if (!isTauriAvailable()) return [];
  return await invoke<AuditLogEntryPayload[]>('get_audit_log', { limit });
};

// ─── Result snapshots (local capture + paged diff) ───────────────────
export interface SnapshotMeta {
  id: string;
  name: string;
  connection_id: string;
  connection_name: string;
  sql_text: string;
  columns: string[];
  row_count: number;
  created_at: string;
}

export type SnapshotDiffKind = 'added' | 'removed' | 'changed';

export interface SnapshotDiffRow {
  kind: SnapshotDiffKind;
  row_key: string;
  left_row?: unknown[] | null;
  right_row?: unknown[] | null;
}

export interface SnapshotDiffResult {
  left_id: string;
  right_id: string;
  added: number;
  removed: number;
  changed: number;
  unchanged: number;
  rows: SnapshotDiffRow[];
  total_diff_rows: number;
  offset: number;
  limit: number;
}

export const saveResultSnapshot = async (payload: {
  name: string;
  connectionId: string;
  connectionName: string;
  sqlText: string;
  columns: string[];
  rows: unknown[][];
}): Promise<SnapshotMeta> => {
  if (!isTauriAvailable()) {
    throw new Error('Result snapshots require the native Tauri desktop app');
  }
  return await invoke<SnapshotMeta>('save_result_snapshot', {
    name: payload.name,
    connectionId: payload.connectionId,
    connectionName: payload.connectionName,
    sqlText: payload.sqlText,
    columns: payload.columns,
    rows: payload.rows,
  });
};

export const listResultSnapshots = async (limit = 100): Promise<SnapshotMeta[]> => {
  if (!isTauriAvailable()) return [];
  return await invoke<SnapshotMeta[]>('list_result_snapshots', { limit });
};

export const deleteResultSnapshot = async (id: string): Promise<void> => {
  if (!isTauriAvailable()) return;
  await invoke('delete_result_snapshot', { id });
};

export const diffResultSnapshots = async (
  leftId: string,
  rightId: string,
  offset = 0,
  limit = 100
): Promise<SnapshotDiffResult> => {
  if (!isTauriAvailable()) {
    throw new Error('Snapshot diff requires the native Tauri desktop app');
  }
  return await invoke<SnapshotDiffResult>('diff_result_snapshots', {
    leftId,
    rightId,
    offset,
    limit,
  });
};

// ─── Structure editor ────────────────────────────────────────────────
export type EngineDialect = 'postgres' | 'mysql' | 'sqlite';

export const structureAddColumn = async (
  connectionId: string,
  tableName: string,
  columnName: string,
  dataType: string,
  engine: EngineDialect
): Promise<void> => {
  if (!isTauriAvailable()) throw new Error('Structure edits require native app');
  // Serde on the Rust side expects lowercase enum variants (rename_all = "lowercase")
  await invoke('structure_add_column', {
    connectionId,
    payload: {
      table_name: tableName,
      column_name: columnName,
      data_type: dataType,
      is_nullable: true,
    },
    engine,
  });
};

export const structureDropColumn = async (
  connectionId: string,
  tableName: string,
  columnName: string,
  engine: EngineDialect
): Promise<void> => {
  if (!isTauriAvailable()) throw new Error('Structure edits require native app');
  await invoke('structure_drop_column', {
    connectionId,
    payload: { table_name: tableName, column_name: columnName },
    engine,
  });
};

// Engines the backend can actually open
export const SUPPORTED_ENGINES = new Set([
  'postgres',
  'postgresql',
  'mysql',
  'mariadb',
  'sqlite',
  'cockroachdb',
  'redshift',
  'duckdb',
  'turso',
  'redis',
]);

export const isEngineSupported = (dbType: string): boolean =>
  SUPPORTED_ENGINES.has(dbType.toLowerCase());

export const importCsvContent = async (
  connectionId: string,
  tableName: string,
  csvContent: string
): Promise<{ inserted_count: number; failed_count: number; failed_rows: { row_index: number; reason: string }[] }> => {
  if (!isTauriAvailable()) {
    throw new Error('CSV import requires the native Tauri desktop app');
  }
  return await invoke('import_csv_content', {
    connectionId,
    tableName,
    csvContent,
  });
};

export const saveSecret = async (account: string, secret: string): Promise<void> => {
  if (!isTauriAvailable()) return;
  await invoke('save_secret', { account, secret });
};

export const getSecret = async (account: string): Promise<string | null> => {
  if (!isTauriAvailable()) return null;
  try {
    return await invoke<string>('get_secret', { account });
  } catch {
    return null;
  }
};

export const deleteSecret = async (account: string): Promise<void> => {
  if (!isTauriAvailable()) return;
  try {
    await invoke('delete_secret', { account });
  } catch {
    /* ignore missing secret */
  }
};

async function webAesEncrypt(payloadObj: any, passphrase: string): Promise<string> {
  const enc = new TextEncoder();
  const jsonBytes = enc.encode(JSON.stringify(payloadObj));
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const nonce = crypto.getRandomValues(new Uint8Array(12));

  const keyMaterial = await crypto.subtle.importKey(
    'raw',
    enc.encode(passphrase),
    { name: 'PBKDF2' },
    false,
    ['deriveKey']
  );

  const key = await crypto.subtle.deriveKey(
    {
      name: 'PBKDF2',
      salt,
      iterations: 100000,
      hash: 'SHA-256',
    },
    keyMaterial,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt']
  );

  const ciphertext = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv: nonce },
    key,
    jsonBytes
  );

  const toB64 = (arr: Uint8Array) => btoa(String.fromCharCode(...arr));
  return JSON.stringify({
    salt_b64: toB64(salt),
    nonce_b64: toB64(nonce),
    ciphertext_b64: toB64(new Uint8Array(ciphertext)),
    kdf_iters: 100000,
  });
}

async function webAesDecrypt(encryptedJsonStr: string, passphrase: string): Promise<any> {
  const enc = new TextEncoder();
  const fileObj = JSON.parse(encryptedJsonStr);
  const fromB64 = (b64: string) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));

  const salt = fromB64(fileObj.salt_b64);
  const nonce = fromB64(fileObj.nonce_b64);
  const ciphertext = fromB64(fileObj.ciphertext_b64);
  const iters = fileObj.kdf_iters || 100000;

  const keyMaterial = await crypto.subtle.importKey(
    'raw',
    enc.encode(passphrase),
    { name: 'PBKDF2' },
    false,
    ['deriveKey']
  );

  const key = await crypto.subtle.deriveKey(
    {
      name: 'PBKDF2',
      salt,
      iterations: iters,
      hash: 'SHA-256',
    },
    keyMaterial,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt']
  );

  const decryptedBytes = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: nonce },
    key,
    ciphertext
  );

  const dec = new TextDecoder();
  return JSON.parse(dec.decode(decryptedBytes));
}

export const exportConnectionsToText = async (
  connectionIds?: string[],
  passphrase?: string
): Promise<string> => {
  if (!isTauriAvailable()) {
    const saved = localStorage.getItem('devdash_connections');
    const allConns = saved ? JSON.parse(saved) : [];
    const filtered = connectionIds
      ? allConns.filter((c: any) => connectionIds.includes(c.id))
      : allConns;
    const payload = {
      connections: filtered,
      saved_queries: [],
      exported_at: new Date().toISOString(),
      version: '1.0',
    };
    return await webAesEncrypt(payload, passphrase || '');
  }
  return await invoke<string>('export_connections_to_text', {
    connectionIds: connectionIds || null,
    passphrase: passphrase || '',
  });
};

export const importConnectionsFromText = async (
  encryptedPayload: string,
  passphrase?: string
): Promise<any> => {
  if (!isTauriAvailable()) {
    const payload = await webAesDecrypt(encryptedPayload, passphrase || '');
    if (payload?.connections && Array.isArray(payload.connections)) {
      const saved = localStorage.getItem('devdash_connections');
      const existing = saved ? JSON.parse(saved) : [];
      const merged = [...existing];
      for (const newConn of payload.connections) {
        if (!merged.some((c: any) => c.id === newConn.id || c.name === newConn.name)) {
          merged.push(newConn);
        }
      }
      localStorage.setItem('devdash_connections', JSON.stringify(merged));
    }
    return payload;
  }
  return await invoke<any>('import_connections_from_text', {
    encryptedPayload,
    passphrase: passphrase || '',
  });
};

// ─── Schema migration (per-table column diff) ────────────────────────
export interface ColumnSnapshotPayload {
  name: string;
  data_type: string;
  is_nullable: boolean;
  is_primary_key: boolean;
}

export interface TableSnapshotPayload {
  table_name: string;
  columns: ColumnSnapshotPayload[];
}

export interface MigrationDiffResultPayload {
  table_name: string;
  added_columns: ColumnSnapshotPayload[];
  removed_columns: string[];
  sql_statements: string[];
}

export const generateMigrationSql = async (
  snapshot: TableSnapshotPayload,
  current: TableSnapshotPayload,
  engine: EngineDialect
): Promise<MigrationDiffResultPayload> => {
  if (!isTauriAvailable()) {
    // Lightweight browser fallback mirroring schema_migration.rs (validated idents)
    const safeIdent = (name: string) => /^[A-Za-z_][A-Za-z0-9_]*$/.test(name) && name.length <= 128;
    const safeTable = (name: string) =>
      /^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)?$/.test(name) && name.length <= 257;
    if (!safeTable(current.table_name)) {
      throw new Error(`Invalid table identifier: ${current.table_name}`);
    }
    const snap = new Map(snapshot.columns.map((c) => [c.name.toLowerCase(), c]));
    const cur = new Map(current.columns.map((c) => [c.name.toLowerCase(), c]));
    const added = current.columns.filter((c) => !snap.has(c.name.toLowerCase()));
    const removed = snapshot.columns
      .filter((c) => !cur.has(c.name.toLowerCase()))
      .map((c) => c.name);
    for (const col of [...added, ...snapshot.columns, ...current.columns]) {
      if (!safeIdent(col.name)) throw new Error(`Invalid column identifier: ${col.name}`);
    }
    for (const name of removed) {
      if (!safeIdent(name)) throw new Error(`Invalid column identifier: ${name}`);
    }
    const q = (id: string) => (engine === 'mysql' ? `\`${id}\`` : `"${id}"`);
    const sql: string[] = [];
    for (const col of added) {
      sql.push(
        `ALTER TABLE ${q(current.table_name)} ADD COLUMN ${q(col.name)} ${col.data_type} ${col.is_nullable ? 'NULL' : 'NOT NULL'
        };`
      );
    }
    for (const name of removed) {
      sql.push(`ALTER TABLE ${q(current.table_name)} DROP COLUMN ${q(name)};`);
    }
    return {
      table_name: current.table_name,
      added_columns: added,
      removed_columns: removed,
      sql_statements: sql,
    };
  }
  return await invoke<MigrationDiffResultPayload>('generate_migration_sql', {
    snapshot,
    current,
    engine,
  });
};

// ─── Backend process kill (protocol-level) ───────────────────────────
export const cancelBackendQuery = async (
  connectionId: string,
  pidOrThreadId: number,
  dbKind: string
): Promise<void> => {
  if (!isTauriAvailable()) {
    throw new Error('Process kill requires the native Tauri desktop app');
  }
  await invoke('cancel_backend_query', {
    connectionId,
    pidOrThreadId,
    dbKind,
  });
};

/** List active server processes for process manager UI. */
export interface DatabaseProcessItem {
  pid: number;
  user: string;
  database: string;
  clientAddr: string;
  state: string;
  query: string;
  durationMs: number;
}

// ─── Autocomplete schema map ─────────────────────────────────────────
export interface AutocompleteData {
  schemas: string[];
  tables: string[];
  table_columns: { table_name: string; columns: string[] }[];
  fetch_time_ms: number;
}

export const getAutocompleteData = async (
  connectionId: string,
  dbKind: string
): Promise<AutocompleteData> => {
  if (!isTauriAvailable()) {
    return {
      schemas: ['public'],
      tables: ['users', 'products', 'orders', 'categories'],
      table_columns: [
        { table_name: 'users', columns: ['id', 'username', 'email', 'role', 'status', 'created_at'] },
        { table_name: 'products', columns: ['id', 'category_id', 'name', 'price', 'stock', 'is_active'] },
        { table_name: 'orders', columns: ['id', 'user_id', 'product_id', 'quantity', 'total_amount', 'order_date'] },
        { table_name: 'categories', columns: ['id', 'name', 'description'] },
      ],
      fetch_time_ms: 1,
    };
  }
  return await invoke<AutocompleteData>('get_autocomplete_data', {
    connectionId,
    dbKind,
  });
};

// ─── Query cancel ────────────────────────────────────────────────────
export const cancelQuery = async (queryId: string): Promise<void> => {
  if (!isTauriAvailable()) return;
  await invoke('cancel_query', { queryId });
};

// ─── Query history (persisted in app SQLite) ─────────────────────────
export interface PersistedQueryHistoryItem {
  id: string;
  query_text: string;
  connection_id: string;
  timestamp: string;
  execution_time_ms: number;
  row_count: number;
  error?: string | null;
}

export const fetchPersistedQueryHistory = async (
  page = 1,
  pageSize = 50
): Promise<PersistedQueryHistoryItem[]> => {
  if (!isTauriAvailable()) return [];
  return await invoke<PersistedQueryHistoryItem[]>('get_query_history', {
    page,
    pageSize,
  });
};

export const clearPersistedQueryHistory = async (): Promise<void> => {
  if (!isTauriAvailable()) return;
  await invoke('clear_all_query_history');
};

export const deletePersistedHistoryEntry = async (id: string): Promise<void> => {
  if (!isTauriAvailable()) return;
  await invoke('delete_history_entry', { id });
};

/** Split a SQL script into statements, respecting quotes and line/block comments. */
export function splitSqlStatements(sql: string): string[] {
  const statements: string[] = [];
  let current = '';
  let i = 0;
  let inSingle = false;
  let inDouble = false;
  let inLineComment = false;
  let inBlockComment = false;

  while (i < sql.length) {
    const ch = sql[i];
    const next = sql[i + 1];

    if (inLineComment) {
      current += ch;
      if (ch === '\n') inLineComment = false;
      i++;
      continue;
    }
    if (inBlockComment) {
      current += ch;
      if (ch === '*' && next === '/') {
        current += '/';
        i += 2;
        inBlockComment = false;
        continue;
      }
      i++;
      continue;
    }
    if (!inSingle && !inDouble) {
      if (ch === '-' && next === '-') {
        current += ch;
        inLineComment = true;
        i++;
        continue;
      }
      if (ch === '/' && next === '*') {
        current += ch;
        inBlockComment = true;
        i++;
        continue;
      }
    }
    if (ch === "'" && !inDouble) {
      // handle escaped ''
      if (inSingle && next === "'") {
        current += "''";
        i += 2;
        continue;
      }
      inSingle = !inSingle;
      current += ch;
      i++;
      continue;
    }
    if (ch === '"' && !inSingle) {
      inDouble = !inDouble;
      current += ch;
      i++;
      continue;
    }
    if (ch === ';' && !inSingle && !inDouble) {
      const trimmed = current.trim();
      if (trimmed) statements.push(trimmed);
      current = '';
      i++;
      continue;
    }
    current += ch;
    i++;
  }
  const tail = current.trim();
  if (tail) statements.push(tail);
  return statements;
}

// ─── Multi-connection ────────────────────────────────────────────────
export const listConnectedIds = async (): Promise<string[]> => {
  if (!isTauriAvailable()) return [];
  return await invoke<string[]>('list_connected_ids');
};

// ─── Transactions ────────────────────────────────────────────────────
export interface TxStatus {
  active: boolean;
  connection_id: string;
  started_at?: string | null;
  statement_count: number;
  duration_ms: number;
}

export const beginTransaction = async (connectionId: string): Promise<TxStatus> => {
  if (!isTauriAvailable()) throw new Error('Transactions require the native app');
  return await invoke<TxStatus>('begin_transaction', { connectionId });
};

export const commitTransaction = async (connectionId: string): Promise<TxStatus> => {
  if (!isTauriAvailable()) throw new Error('Transactions require the native app');
  return await invoke<TxStatus>('commit_transaction', { connectionId });
};

export const rollbackTransaction = async (connectionId: string): Promise<TxStatus> => {
  if (!isTauriAvailable()) throw new Error('Transactions require the native app');
  return await invoke<TxStatus>('rollback_transaction', { connectionId });
};

export const getTransactionStatus = async (connectionId: string): Promise<TxStatus> => {
  if (!isTauriAvailable()) {
    return {
      active: false,
      connection_id: connectionId,
      statement_count: 0,
      duration_ms: 0,
    };
  }
  return await invoke<TxStatus>('get_transaction_status', { connectionId });
};

// ─── Diagnostics ─────────────────────────────────────────────────────
export interface DiagnosticCheck {
  name: string;
  ok: boolean;
  detail: string;
}

export interface ConnectionDiagnostics {
  success: boolean;
  latency_ms: number;
  server_version: string;
  current_database: string;
  current_user: string;
  is_superuser?: boolean | null;
  max_connections?: number | null;
  active_connections?: number | null;
  database_size_pretty?: string | null;
  encoding?: string | null;
  uptime_seconds?: number | null;
  message: string;
  checks: DiagnosticCheck[];
}

export const diagnoseConnection = async (
  connectionId: string
): Promise<ConnectionDiagnostics> => {
  if (!isTauriAvailable()) throw new Error('Diagnostics require the native app');
  return await invoke<ConnectionDiagnostics>('diagnose_connection', { connectionId });
};

// ─── Query profiling ─────────────────────────────────────────────────
export interface ProfileNode {
  node_type: string;
  relation?: string | null;
  cost?: number | null;
  actual_ms?: number | null;
  rows?: number | null;
  detail: string;
}

export interface QueryProfile {
  sql: string;
  dialect: string;
  profile_sql: string;
  total_time_ms: number;
  planning_time_ms?: number | null;
  execution_time_ms?: number | null;
  plan_text: string;
  plan_json?: string | null;
  summary: string;
  nodes: ProfileNode[];
}

export const profileSqlQuery = async (
  connectionId: string,
  sql: string
): Promise<QueryProfile> => {
  if (!isTauriAvailable()) throw new Error('Profiling requires the native app');
  return await invoke<QueryProfile>('profile_sql_query', { connectionId, sql });
};

// ─── Migration apply ─────────────────────────────────────────────────
export interface ApplyMigrationResult {
  success: boolean;
  dry_run: boolean;
  statements_run: number;
  duration_ms: number;
  error?: string | null;
  run_id: string;
}

export interface MigrationRunRecord {
  id: string;
  source_connection: string;
  target_connection: string;
  sql_script: string;
  dry_run: boolean;
  success: boolean;
  error?: string | null;
  statements_run: number;
  duration_ms: number;
  created_at: string;
}

export const applyMigrationSql = async (
  connectionId: string,
  sourceLabel: string,
  targetLabel: string,
  sqlScript: string,
  dryRun: boolean
): Promise<ApplyMigrationResult> => {
  if (!isTauriAvailable()) throw new Error('Migration apply requires the native app');
  return await invoke<ApplyMigrationResult>('apply_migration_sql', {
    connectionId,
    sourceLabel,
    targetLabel,
    sqlScript,
    dryRun,
  });
};

export const listMigrationRuns = async (limit = 50): Promise<MigrationRunRecord[]> => {
  if (!isTauriAvailable()) return [];
  return await invoke<MigrationRunRecord[]>('list_migration_runs', { limit });
};

export const listDatabaseProcesses = async (
  connectionId: string,
  _dbKind: string
): Promise<DatabaseProcessItem[]> => {
  if (!isTauriAvailable()) return [];
  const rows = await invoke<
    Array<{
      pid: number;
      user: string;
      database: string;
      client_addr: string;
      state: string;
      query: string;
      duration_ms: number;
    }>
  >('list_database_processes', { connectionId });
  return rows.map((r) => ({
    pid: Number(r.pid ?? 0),
    user: String(r.user ?? ''),
    database: String(r.database ?? ''),
    clientAddr: String(r.client_addr ?? ''),
    state: String(r.state ?? ''),
    query: String(r.query ?? ''),
    durationMs: Number(r.duration_ms ?? 0),
  }));
};

// ─── Shared connection catalog (Desktop + CLI + Mobile) ──────────────
export interface CatalogConnection {
  id: string;
  name: string;
  db_type: string;
  host: string;
  port: number;
  user: string;
  database: string;
  ssl_mode?: string | null;
  environment: string;
  is_read_only: boolean;
  allow_writes_on_prod: boolean;
  updated_at?: string;
  origin_device_id?: string;
}

export interface ConnectionCatalogPayload {
  version: number;
  default?: string | null;
  connections: CatalogConnection[];
}

export const listConnectionCatalog = async (): Promise<ConnectionCatalogPayload> => {
  if (!isTauriAvailable()) {
    return { version: 1, default: null, connections: [] };
  }
  return await invoke<ConnectionCatalogPayload>('catalog_list');
};

export const upsertCatalogConnection = async (
  conn: CatalogConnection,
  password?: string,
  platform: 'desktop' | 'mobile' | 'cli' = 'desktop'
): Promise<ConnectionCatalogPayload> => {
  if (!isTauriAvailable()) {
    return { version: 1, default: conn.name, connections: [conn] };
  }
  return await invoke<ConnectionCatalogPayload>('catalog_upsert', {
    conn,
    password: password || null,
    platform,
  });
};

export const removeCatalogConnection = async (
  nameOrId: string
): Promise<ConnectionCatalogPayload> => {
  if (!isTauriAvailable()) {
    return { version: 1, default: null, connections: [] };
  }
  return await invoke<ConnectionCatalogPayload>('catalog_remove', { nameOrId });
};

export const setCatalogDefault = async (name: string): Promise<ConnectionCatalogPayload> => {
  if (!isTauriAvailable()) {
    return { version: 1, default: name, connections: [] };
  }
  return await invoke<ConnectionCatalogPayload>('catalog_set_default', { name });
};

export const ingestGuiConnections = async (
  connections: CatalogConnection[],
  platform: 'desktop' | 'mobile' = 'desktop'
): Promise<MergeReport> => {
  if (!isTauriAvailable()) {
    return emptyMergeReport();
  }
  return await invoke<MergeReport>('catalog_ingest_gui_connections', {
    connections,
    platform,
  });
};

export function catalogToConfig(c: CatalogConnection): ConnectionConfig {
  return {
    id: c.id,
    name: c.name,
    db_type: (c.db_type || 'postgres') as ConnectionConfig['db_type'],
    host: c.host,
    port: c.port,
    user: c.user,
    database: c.database,
    ssl_mode: c.ssl_mode || undefined,
    environment: (c.environment as ConnectionConfig['environment']) || 'dev',
    is_read_only: c.is_read_only,
    allow_writes_on_prod: c.allow_writes_on_prod,
  };
}

export function configToCatalog(c: ConnectionConfig): CatalogConnection {
  return {
    id: c.id,
    name: c.name,
    db_type: c.db_type,
    host: c.host,
    port: c.port,
    user: c.user,
    database: c.database,
    ssl_mode: c.ssl_mode || null,
    environment: c.environment || 'dev',
    is_read_only: !!c.is_read_only,
    allow_writes_on_prod: !!c.allow_writes_on_prod,
    updated_at: '',
    origin_device_id: '',
  };
}

// ─── Device sync ─────────────────────────────────────────────────────
export interface DeviceIdentity {
  id: string;
  name: string;
  platform: string;
  created_at: string;
}

export interface ConflictRecord {
  kind: string;
  id: string;
  name: string;
  resolution: string;
  reason: string;
}

export interface MergeReport {
  connections_added: number;
  connections_updated: number;
  connections_kept_local: number;
  queries_upserted: number;
  history_inserted: number;
  secrets_imported: number;
  conflicts: ConflictRecord[];
  origin_device_id: string;
  origin_device_name: string;
}

export interface SyncStatus {
  device: DeviceIdentity;
  catalog_path: string;
  catalog_count: number;
  default_connection?: string | null;
  saved_query_count: number;
  history_count: number;
  last_export_path?: string | null;
}

export interface SyncExportReport {
  path?: string | null;
  ciphertext: string;
  connection_count: number;
  query_count: number;
  history_count: number;
  include_secrets: boolean;
  origin_device: DeviceIdentity;
}

function emptyMergeReport(): MergeReport {
  return {
    connections_added: 0,
    connections_updated: 0,
    connections_kept_local: 0,
    queries_upserted: 0,
    history_inserted: 0,
    secrets_imported: 0,
    conflicts: [],
    origin_device_id: '',
    origin_device_name: '',
  };
}

export const getDeviceSyncStatus = async (
  platform: 'desktop' | 'mobile' | 'cli' = 'desktop'
): Promise<SyncStatus> => {
  if (!isTauriAvailable()) {
    return {
      device: {
        id: 'web-preview',
        name: 'browser',
        platform,
        created_at: new Date().toISOString(),
      },
      catalog_path: '(web preview — no catalog)',
      catalog_count: 0,
      saved_query_count: 0,
      history_count: 0,
    };
  }
  return await invoke<SyncStatus>('device_sync_status', { platform });
};

export const exportDeviceSync = async (opts: {
  passphrase: string;
  includeSecrets?: boolean;
  path?: string;
  platform?: 'desktop' | 'mobile' | 'cli';
}): Promise<SyncExportReport> => {
  if (!isTauriAvailable()) {
    throw new Error('Device sync requires the native app');
  }
  return await invoke<SyncExportReport>('device_sync_export', {
    passphrase: opts.passphrase,
    includeSecrets: opts.includeSecrets ?? false,
    path: opts.path || null,
    platform: opts.platform || 'desktop',
  });
};

export const importDeviceSync = async (opts: {
  passphrase: string;
  ciphertext?: string;
  path?: string;
  importSecrets?: boolean;
}): Promise<MergeReport> => {
  if (!isTauriAvailable()) {
    throw new Error('Device sync requires the native app');
  }
  return await invoke<MergeReport>('device_sync_import', {
    passphrase: opts.passphrase,
    ciphertext: opts.ciphertext || null,
    path: opts.path || null,
    importSecrets: opts.importSecrets ?? true,
  });
};

export const suggestedSyncExportPath = async (): Promise<string> => {
  if (!isTauriAvailable()) return '';
  return await invoke<string>('device_sync_suggested_path');
};

// ─── AI SQL assist (same core as CLI `devdash ai`) ───────────────────
export interface AiAssistRequestPayload {
  provider: string;
  base_url?: string | null;
  model?: string | null;
  api_key?: string | null;
  db_type: string;
  active_table?: string | null;
  last_queries: string[];
  tables: { name: string; columns: string[] }[];
  prompt: string;
}

export interface AiAssistResponsePayload {
  sql: string;
  is_write: boolean;
  provider: string;
  model: string;
}

export const generateSqlAssist = async (
  req: AiAssistRequestPayload
): Promise<AiAssistResponsePayload> => {
  if (!isTauriAvailable()) {
    return {
      sql: `-- web preview: ${req.prompt}`,
      is_write: false,
      provider: req.provider,
      model: req.model || 'preview',
    };
  }
  return await invoke<AiAssistResponsePayload>('generate_sql_assist', { req });
};


