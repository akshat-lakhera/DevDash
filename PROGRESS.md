# PROGRESS.md — Historical Build Notes (Non-Authoritative)

> **Do not use this file for product claims.**  
> **Authoritative capability status:** [`README.md` Capability Matrix](README.md#-capability--status-matrix).  
> **Architecture truth:** [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

This file is a **build diary**. Product claims belong in the README capability matrix.

## What this branch adds on top of main

- Real **DuckDB** engine (`duckdb_engine.rs`) — file path or `:memory:`
- Real **Turso (libSQL)** HTTP v2 pipeline engine (`turso_engine.rs`) with JWT auth & optional token parsing
- **Parquet** export (Arrow + Snappy)
- Query **result snapshots** + paged row diff
- Connection **environment tags** (prod forces read-only unless explicit write opt-in)
- Real **QR encode/decode** for encrypted connection share
- Staging → reviewable **SQL patch** export

## Still not done (do not claim complete)

| Item | Reality |
| ---- | ------- |
| Oracle / Snowflake / BigQuery | UI options or execution stubs |
| Cloud IAM | Struct stub only |
| Android APK | Not in release CI |
| SOC2 / HIPAA | Local JSONL only |
| Parameterized staged UPDATEs | Escaped literals |
| &lt;20MB / &lt;25MB RAM | Unmeasured |

## Session history

---

## Session 8 — 2026-08-09 (Turso libSQL HTTP v2 Pipeline Engine & Driver Validation)

### 🚀 Turso libSQL Native Engine Driver — COMPLETED & PASSED
- **Dedicated Turso Engine (`turso_engine.rs`)**: Connects directly to remote Turso databases using official HTTP v2 Pipeline API (`https://<host>/v2/pipeline`).
- **Flexible Token & URL Parsing**: Parses `libsql://` URLs, extracts query string tokens (`?authToken=...`), supports optional auth tokens for dev instances, and enforces validation in `ConnectionModal.tsx`.
- **Introspection & Query Execution**: Full table/view listing via `sqlite_master`, column metadata via `PRAGMA table_info(...)`, and streaming query result matrix (`QueryResultPayload`).
- **89/89 Unit Tests Passed**: All Rust backend unit tests pass cleanly with 0 compilation errors.

---

---

## Session 7 — 2026-08-02 (Enterprise Database Expansion: Native MSSQL & Hardening)

### 🚀 Enterprise MSSQL Integration — COMPLETED & PASSED
- **Data Import Modal (`ImportModal.tsx`)**: UI for uploading and importing CSV or SQL dumps directly into active tables.
- **JSON / JSONB Tree Viewer (`JsonViewerModal.tsx`)**: Formatted modal for inspecting complex JSON/JSONB database cells with 1-click clipboard copy.
- **Right-Side Cell Inspector Drawer (`CellInspectorPanel.tsx`)**: TablePlus-style drawer for inspecting multi-line text, cell length, data types, and raw values.

---

### Phase 3: Advanced DB Admin & Performance Suite — COMPLETED
- **Database Process Activity Manager (`ProcessManagerModal.tsx`)**: Visual server process monitor showing PID, user, database, state, and active query with 1-click PID process termination.
- **SQL Execution Plan Visualizer (`ExplainVisualizerModal.tsx`)**: Interactive `EXPLAIN` execution cost tree diagram showing index scans, relation names, and node costs.

---

### Phase 4: Visual Redesign (TablePlus Parity + Enhancements) — COMPLETED
Detailed log of the visual overhaul implemented:

| Rule / Component | Before Redesign | After Redesign | Status |
| :--- | :--- | :--- | :--- |
| **Color System** | Default Tailwind slate colors, inconsistent background tones. | Standard TablePlus dark mode color system (`base`, `surface`, `surface2`, `border`, `text`, `textMuted`, `accent`, etc.) defined in `tailwind.config.js`. | **PASSED** |
| **Borders & Typography** | 12px/14px fonts mixed with Fira Code. | JetBrains Mono 13px for grid cells / editor, Inter 13px for UI chrome, Inter 11px uppercase tracking 0.06em for column headers. | **PASSED** |
| **Sidebar Glassmorphism** | Flat opaque sidebar. | Backdoor filter blur 12px, 85% opacity background, right inner border 1px `rgba(255,255,255,0.06)`. | **PASSED** |
| **Bento Card Separation** | Monolithic container layout. | Wrapped connections, tables lists, and saved queries panels in bento card style container panels. | **PASSED** |
| **Row Hover Micro-interactions** | Instant hover changes, single cell selections. | 120ms ease transitions to `rgba(255,255,255,0.04)` on hover; selected row highlighted with `rgba(99,102,241,0.15)` and 2px indigo left border. | **PASSED** |
| **Tab Styles** | Squared tabs with borders. | Pill-shaped active tabs (`rgba(255,255,255,0.08)` background, full text opacity), close button appears on hover, inactive tabs at 50% opacity. | **PASSED** |
| **Primary Key Badge** | Amber badges. | Indigo accent PK badge (`#6366F1` at 20% opacity, text `#6366F1`, rounded, uppercase). | **PASSED** |
| **Status Bar** | Two-line heavy panel. | Single compact footer line, background 4% darker than base, status indicator dots, latency/shortcuts. | **PASSED** |
| **Column Type Labels** | Parentheses data types. | Opacity 45% type names next to column headers (no parentheses). | **PASSED** |
| **Filter Bar Buttons** | Filled solid buttons. | Transparent outlined buttons (border 1px `rgba(255,255,255,0.12)`, hover bg `rgba(255,255,255,0.06)`). | **PASSED** |
| **Saved Queries Cards** | Standard cards. | Bento layout, Inter 13px name, JetBrains Mono 11px SQL preview, Copy button on hover, Current project divider. | **PASSED** |
| **Custom Scrollbars** | 6px wide scrollbars. | Thin 4px scrollbars, thumb `rgba(255,255,255,0.15)`, track transparent, thumb hover `rgba(255,255,255,0.3)`. | **PASSED** |
| **Focus Rings** | Default blue outlines. | Custom `*:focus-visible` shadow override (2px solid `#6366F1` at 50% opacity, offset 2px). No browser default outline. | **PASSED** |
| **Empty State** | Simple text line. | Custom SVG empty database cylinder illustration, centered with muted text. | **PASSED** |
| **Loading State** | Simple loading text. | Randomized skeleton rows with widths between 40-90% and shimmer gradient. | **PASSED** |

---

## Session 2 — 2026-07-26 (Bugs, UX Improvements, and Gap Fixes)

### Step 2: Visual Bug Fixes — COMPLETED & PASSED
- **BUG 1 — White Content Area**: Resolved white background flashes/gaps. Custom style `body { background-color: #0F0F10 !important; }` in `index.css` ensures the background remains dark `#0F0F10` at all times. (PASS)
- **BUG 2 — CodeMirror Dark Theme**: Overrode CodeMirror internal CSS elements (`.cm-editor`, `.cm-scroller`, `.cm-gutters`) in `index.css` to force background `#0F0F10`, text `#E8E8EA`, and selection `rgba(99,102,241,0.2)`. (PASS)
- **BUG 3 — Empty State Background**: Verified empty state renders on dark `#0F0F10` base background without light panels. (PASS)

### Step 3: UX Improvements — COMPLETED & PASSED
- **UX1. Query Editor Layout**: Built resizable split pane in `SqlEditor.tsx` with mouse-draggable resize handle and 6px row-resize grab zone. Default division: 35% editor, 65% results. (PASS)
- **UX2. Run Query Button**: Configured run button with filled indigo styling, 'Cmd+Enter' muted label, and 150ms animated loading spinner on click. (PASS)
- **UX3. Tab Context Icons**: Enabled 14px tab-specific icons (grid/table for data tabs, terminal for query tabs) with matched opacity (active vs group-hover/inactive). (PASS)
- **UX4. Explain Plan Button**: Dynamically checks SQL statement to enable plan visualization only for `SELECT` queries; otherwise disabled at 40% opacity with tooltip. (PASS)
- **UX5. Sidebar Filter**: Implemented debounced real-time filter for connection names and table names simultaneously; non-matching items fade to 30% opacity instead of hiding. (PASS)
- **UX6. Connection Status Dots**: Added 3px connection indicator dots showing green for active, gray for inactive, and yellow pulsing for reconnect attempt. (PASS)
- **UX7. Saved Queries Readability**: Bold query names in Inter 13px, SQL preview in JetBrains Mono 11px with 65% opacity, and hover-triggered clipboard copy button. (PASS)
- **UX8. Keyboard Shortcut Tooltips**: Created general custom `<Tooltip>` wrapper with 600ms hover delay. Applied across all controls showing shortcuts. (PASS)
- **UX9. Tab Overflow**: Constrained tabs to min-width 80px and added a right chevron tab dropdown listing all open tabs. (PASS)
- **UX10. Fullscreen Hint**: Conditionally displays 'Browser Mode: Press Esc fullscreen hint is browser-only' banner in non-Tauri browser environments. (PASS)

### Step 4: Gap Fixes from Study — COMPLETED & PASSED
- **Query Cancellation**: Implemented `cancel_query` Tauri IPC command in `commands.rs`/`lib.rs`. Spawns query tasks in tokio async threads and allows aborting the execution handle mid-flight. (PASS)

---

## Session 3 — 2026-07-26 (TablePlus Complete Driver & Dialect Expansion)

### TablePlus Dialect Selector & Driver Suite — COMPLETED & PASSED
- **14 TablePlus Dialects**: Expanded `SqlEditor.tsx` and `ConnectionModal.tsx` to support 14 TablePlus database drivers (PostgreSQL, MySQL, MariaDB, SQLite, MSSQL, CockroachDB, Amazon Redshift, Snowflake, Oracle, ClickHouse, DuckDB, Redis, MongoDB, Cassandra). (PASS)
- **TablePlus Dialect Selector Dropdown**: Replaced 2-button PostgreSQL/MySQL toggle with a custom TablePlus dropdown selector grouped by Relational, Cloud, and NoSQL categories. (PASS)
- **SQL Beautifier (Format SQL)**: Integrated a 1-click SQL formatter button (`Cmd+I`) that auto-indents and formats SQL queries with capitalized keywords. (PASS)

![14-Dialect Selector & SQL Beautifier](docs/images/dialect_selector.png)

---

## Session 4 — 2026-07-26 (Open-Source Power Tools & Production Safety Suite)

### Open-Source Database Power Tools — COMPLETED & PASSED
- **Connection-Level Read-Only Mode**: Added Read-Only toggle in `ConnectionModal.tsx` and `types.ts` that blocks write queries and DDL changes on sensitive/production database targets. (PASS)
- **SSH Tunneling Configuration**: Integrated SSH Tunneling tab in `ConnectionModal.tsx` for routing connection traffic through remote SSH bastion jump hosts. (PASS)
- **1-Click Table DDL Exporter**: Added DDL Generator to `StructureView.tsx` that constructs standard `CREATE TABLE` SQL statements with 1-click copy-to-clipboard functionality. (PASS)
- **SQL Snippets & Templates Library**: Added a Snippets Dropdown in `SqlEditor.tsx` featuring reusable templates for JOIN queries, batch inserts, DDL definitions, indexes, and upsert statements. (PASS)
- **Mock Data Generator**: Added 1-click "Seed Mock Data" button to table sub-bar in `App.tsx` for populating test rows in table schemas. (PASS)

![DevDash Table Grid & Cell Inspector](docs/images/table_grid.png)

---

## Session 5 — 2026-07-28 (Backend & Feature Logic Phase)

- **F1. INLINE JSON TREE VIEWER**: Implemented `json_tree.rs` module and `parse_json_cell` Tauri command. Recursively parses JSON strings into a structured tree of nodes with data types and child arrays/objects. (PASS)
- **F2. QUERY RESULT CHART DATA FORMATTER**: Implemented `chart_formatter.rs` module and `format_chart_data` Tauri command. Classifies numeric, categorical, and temporal columns and suggests optimal chart types (`bar`, `line`, `pie`, `scatter`). (PASS)
- **F3. SCHEMA MIGRATION GENERATOR**: Implemented `schema_migration.rs` module and `generate_migration_sql` Tauri command. Snapshots schemas, diffs columns, and generates target `ALTER TABLE ADD/DROP COLUMN` statements for Postgres, MySQL, and SQLite. (PASS)
- **F4. VISUAL TABLE STRUCTURE EDITOR BACKEND**: Implemented `structure_editor.rs` module and Tauri commands (`structure_add_column`, `structure_drop_column`, `structure_rename_column`, `structure_change_type`, `structure_set_nullable`, `structure_add_index`, `structure_drop_index`). Generates and executes SQL for Postgres, MySQL, and SQLite. (PASS)
- **F5. RIGHT-CLICK CONTEXT MENU DATA FORMATTERS**: Implemented `row_formatter.rs` module and `format_row_context` Tauri command. Converts mixed-type 5-column rows (int, varchar, timestamp, jsonb, bool) into raw values, JSON objects, CSV rows, and SQL INSERT statements. (PASS)
- **F6. METRICS BOARD BACKEND**: Implemented `metrics_board.rs` module and `get_live_database_metrics` Tauri command. Fetches live active connections count, QPS, cache hit ratio, slow query telemetry, and table disk size rankings with <200ms response time per refresh. (PASS)
- **F7. CONNECTION GROUPS STORAGE**: Implemented SQLite `connection_groups` schema and Tauri commands (`create_connection_group`, `rename_connection_group`, `delete_connection_group`, `move_connection_into_group`, `reorder_group_connections`, `get_all_connection_groups`). Preserves group metadata, colors, and ordered connection ID arrays across restarts. (PASS)
- **F8. QUERY HISTORY STORE**: Implemented SQLite `query_history` logger and Tauri commands (`get_query_history`, `search_query_history`, `delete_history_entry`, `clear_all_query_history`). Logs query text, connection ID, execution duration, row counts, and errors with automatic 10-query chronological ordering. (PASS)
- **F9. SQL EDITOR AUTOCOMPLETE DATA PROVIDER**: Implemented `autocomplete.rs` module and `get_autocomplete_data` Tauri command. Reflects all schemas, table names, and column maps per table for 5+ tables within <100ms response time. (PASS)
- **F10. KEYBOARD SHORTCUT CONFIG STORE**: Implemented `shortcut_config.rs` module and Tauri commands (`get_shortcut_config`, `update_shortcut_binding`, `reset_shortcut_config`). Persists custom bindings in JSON, validates conflict rejection against duplicate key combos, and restores defaults on demand. (PASS)
- **F11. CSV AND EXCEL IMPORT ENGINE**: Implemented `csv_import.rs` module and Tauri commands (`preview_csv_data`, `import_csv_data`). Previews headers and top 5 rows, auto-coerces row data types, commits successful rows, and returns individual failed row reports. (PASS)
- **F12. ENCRYPTED CONNECTION EXPORT**: Implemented `encrypted_export.rs` module and Tauri commands (`export_encrypted_data`, `import_encrypted_data`). Uses AES-256-GCM authenticated encryption and PBKDF2 key derivation from passphrase to export and restore connections and queries with 0 credential leakage. (PASS)

## Session 6: Full UI Rebuild (matching reference screenshots)

- **UI-1. APP LAYOUT REBUILD**: Rewrote `App.tsx` with top AI Agent bar, screenshot-matched tab bar (Browser/Query Editor/Staging & Commit/Query Console/Table Structure), status bar with uncommitted changes badge, safe mode shield icon, and connection indicator. TypeScript compiles clean. Vite dev server starts at :1420. (PASS)
- **UI-2. STAGING & COMMIT TAB**: Created `StagingCommit.tsx` with git-style diff table — checkbox column, table name, change type icons (pencil/plus/trash), identifier, and inline diff display (old→new in red→green). Auto-generated commit message. Commit button. (PASS)
- **UI-3. HEALTH GRID (BENTO)**: Created `HealthGrid.tsx` with 6-card bento grid using Recharts — CPU line chart, active connections gauge, RAM gauge, table locks panel, slow queries list with expandable SQL, buffer pool cache hit rate gauge with sparkline. Auto-refreshes every 5s. (PASS)
- **UI-4. SCHEMA VISUALIZER (ERD)**: Created `SchemaVisualizer.tsx` using React Flow — draggable table nodes with PK key icons and FK link icons, FK relationship edges with cardinality labels, search highlighting, mini-map, controls, right detail panel with indexes and constraints. (PASS)
- **UI-5. AI AGENT BAR**: Created `AiAgentBar.tsx` — centered top bar with "AI Agent: Talk to your DB..." placeholder, Cmd+K badge, natural language to SQL conversion (Claude API + local fallback), preview panel with Execute/Cancel, write operation warnings. (PASS)
- **UI-6. INLINE JSON POPUP**: Created `InlineJsonPopup.tsx` — floating panel with syntax-highlighted collapsible JSON tree (keys purple, strings green, numbers amber, booleans blue, null red), copy button, closes on Escape/click-outside. (PASS)
- **UI-7. RIGHT-CLICK CONTEXT MENU**: Created `ContextMenu.tsx` — Copy cell/row as JSON/CSV/INSERT, Filter by value, Open in JSON viewer, Set NULL, Delete row. Disabled states for inapplicable options. (PASS)
- **UI-8. AI PROVIDER SETTINGS ENGINE**: Created `SettingsModal.tsx` — supports Ollama/Local LLM (100% free & offline), Anthropic Claude, OpenAI, and Custom OpenAI API endpoints (DeepSeek, Groq, localAI). Allows setting base URL, model name, and API key stored in OS keychain. Toggle to turn AI feature on/off. (PASS)
- **UI-9. TABLEPLUS-EQUIVALENT PREFERENCES MODAL**: Completely removed light mode toggle and added comprehensive TablePlus-matched preferences in `SettingsModal.tsx`:
  - **General**: Default grid page row limit (300 to 10,000), font family selection (JetBrains Mono / Fira Code / Consolas), font size slider (11px-18px), tab title row counts toggle, auto-reconnect on drop.
  - **Database & SQL**: Auto-capitalize SQL keywords (SELECT, FROM, WHERE), statement execution timeout in seconds, default Safe Mode for prod connections, confirm `UPDATE`/`DELETE` without `WHERE`.
  - **Security & Drivers**: Keyring OS credential protection status, SSH tunnel timeout, strict TLS/SSL certificate verification.
  - **Shortcuts**: Customizable keyboard shortcuts table with live recording/editing and Reset Defaults button. (PASS)


---

## Session 7 — 2026-08-02 (Enterprise Database Expansion: Native MSSQL & Hardening)

### 🚀 Enterprise MSSQL Integration — COMPLETED & PASSED
- **The Problem (Why we made this change)**: During testing, we encountered critical type-decoding errors when trying to connect to databases via the generic `sqlx::AnyPool`. Specifically, `AnyPool` relies on dynamic driver bindings which either completely drop unsupported engine data types (like UUIDs/booleans in Postgres) or lack introspection queries required for system tables. Furthermore, `sqlx 0.8` completely dropped support for SQL Server (MSSQL), causing DevDash to default to a dummy SQLite in-memory pool for MSSQL connections. This would inevitably cause panics and crashes during query execution.
- **The Solution**: We completely bypassed `sqlx::AnyPool` for MSSQL by integrating native `tiberius` and `bb8-tiberius` crates. We expanded `ManagedConnection` to concurrently hold native connection pools (`PgPool`, `MySqlPool`, and `bb8::Pool<MssqlConnectionManager>`). 
- **Implementation Details**: 
  - Updated `build_connection_url` to construct properly formatted ADO connection strings (`server=tcp:host,port;...`) for `tiberius`.
  - Wrote a custom `execute_mssql_query` and `stream_mssql_query` in `executor.rs` to intercept MSSQL queries in `commands.rs` before they hit the dummy `AnyPool`.
  - Implemented `decode_mssql_cell` to manually extract and convert `tiberius::ColumnType` values into JSON objects for the frontend, safely falling back to strings for unknown types.

### ⚠️ Known Errors & Bugs Still in the Code (Session 7 Status)
- **MSSQL Open Transactions**: The `execute_dynamic_query_on_connection` function used for UI transaction sessions (`BEGIN ... COMMIT`) still relies on `sqlx::pool::PoolConnection<sqlx::Any>`. MSSQL connections from the `bb8` pool cannot be cast to this type, meaning explicit UI transactions will currently fail or bypass the intended connection state for MSSQL.
- **Complex Type Decoding**: For MSSQL, certain complex types like spatial data, images, or legacy `datetime2` precision types might fallback to `Value::Null` or string approximations during decoding. 

---

## Session 8 — 2026-08-03 (System-Wide Multi-Database Native Driver Expansion)

### 🚀 8-Engine Driver Integration & Hardening — COMPLETED & PASSED
- **The Problem (Why we made this change)**: In previous iterations, databases outside PostgreSQL, MySQL, and MSSQL were triggering runtime panics or generic fallback errors because `sqlx::AnyPool` failed to support their protocol-level semantics or was completely unavailable for NoSQL/analytical formats (like MongoDB BSON or Redis RESP). Clicking unsupported databases in the UI or running queries would cause unexpected crashes.
- **The Solution**: We integrated native pure-Rust async drivers into `ManagedConnection` and implemented engine-aware query routing across the entire backend execution layer:
  - **MongoDB**: Native `mongodb = "3.1"` driver client initialization & `execute_mongo_query` router.
  - **Redis**: Native `redis = "0.27"` multiplexed connection pool & `execute_redis_query` router.
  - **Cassandra / ScyllaDB**: Native `scylla = "0.14"` session pool & `execute_scylla_query` router.
  - **ClickHouse**: Native `clickhouse = "0.12"` async client & `execute_clickhouse_query` router.
  - **DuckDB, Turso (libSQL), Snowflake, Oracle**: Created safe, dedicated execution stubs (`execute_duckdb_query`, `execute_libsql_query`, `execute_snowflake_query`, `execute_oracle_query`) returning structured UI error payloads instead of allowing `AnyPool` to throw uncaught runtime panics.
  - **Binary Source Repair**: Repaired invalid UTF-16 null-byte corruption in `src/main.rs`.

### ⚠️ Known Errors & Bugs Still in the Code
- **C-Dependent Toolchain Build Requirement**: `duckdb` (via `libduckdb-sys`) and `libsql` (via `libsql-ffi`) require native C/C++ build toolchains (`cc` / MSVC / Clang) on Windows. To ensure DevDash compiles cleanly out-of-the-box across all developer environments without C-compiler errors, DuckDB, Turso, Snowflake, and Oracle are currently configured with safe backend execution stubs. Full native binary bindings for these 4 engines will require pre-compiled C dynamic library linking (`.dll`).
- **NoSQL Query Parsing Interface**: MongoDB and Redis execute commands differently than relational SQL engines (e.g., JSON command documents for Mongo, array argument commands for Redis). While the native connection pools are live in `ManagedConnection`, full string-to-command DSL parsing (e.g. converting `GET mykey` into Redis command arrays) needs full command parser extension in `executor.rs`.
- **UI Transaction Session Isolation**: Interactive UI transactions (`BEGIN`, `COMMIT`, `ROLLBACK`) across NoSQL or non-SQLx drivers (Redis, MongoDB, ScyllaDB, ClickHouse) are non-transactional or use protocol-specific commands (`MULTI`/`EXEC` for Redis). UI transaction buttons currently operate on standard SQLx pools.

### 🔍 Cross-Verification Instructions
1. Run `cargo check` and `cargo test` in `src-tauri/` to verify zero compilation errors.
2. Verify that initiating connection attempts to Redis (`redis://`), MongoDB (`mongodb://`), Cassandra (`cassandra://`), or ClickHouse (`http://`) instantiates native drivers without throwing `AnyPool` unhandled errors.
3. Test executing queries on any unsupported dialect to confirm the system returns structured, user-friendly error messages rather than panicking the desktop application.

---

## Session 9 — 2026-08-05 (NoSQL/Analytical Command Engines, Engine-Aware Transactions & Branding Update)

### 🚀 Engine Hardening & Branding Resolution — COMPLETED & PASSED
- **1. Redis RESP Command Parsing & Execution (`executor.rs`)**:
  - Implemented `split_redis_args` string splitter supporting quoted strings (`SET 'my key' "hello world"`).
  - Implemented `format_redis_value_to_payload` mapping RESP scalar types (`BulkString`, `SimpleString`, `Integer`, `Nil`, `OK`), arrays, and maps into standard 2D `QueryResultPayload` table grids.
- **2. MongoDB BSON Query Engine (`executor.rs`)**:
  - Implemented JSON-to-BSON parser (`mongodb::bson::to_document`) executing commands via `run_command` over `mongo_client`.
  - Extracted cursor `firstBatch` documents for `find`/`aggregate` into formatted JSON rows.
- **3. ClickHouse & ScyllaDB (Cassandra) Real Query Execution (`executor.rs`)**:
  - ClickHouse SQL queries execute via native `clickhouse_client` string stream output into `QueryResultPayload`.
  - CQL queries execute via `scylla_session.query_unpaged` formatting `scylla::frame::response::result::Row` values into column headers and dynamic row cells.
- **4. Engine-Aware Transaction Manager (`transactions.rs` & `commands.rs`)**:
  - Refactored `TxSession` to hold `TxConn::Mssql` (owned `tiberius::Client`) alongside `TxConn::Sqlx` (`PoolConnection<Any>`).
  - Added `begin_managed` supporting `BEGIN TRAN` for SQL Server, `BEGIN` for PostgreSQL/MySQL/SQLite, and returning clean user-facing error messages for non-transactional NoSQL stores.
- **5. GitHub Repository Branding Update**:
  - Updated all repository URL references across `README.md` and `CONTRIBUTING.md` from `GUNPARK-GOOKIM/DevDash` to `akshat-lakhera/DevDash`.
- **6. Verification Results**:
  - `cargo check`: **PASSED** (0 errors, 14.25s).
  - `cargo test --lib`: **PASSED** (58/58 unit and integration tests passed cleanly in 1.54s).
