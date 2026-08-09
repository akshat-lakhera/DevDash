import React, { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import {
  Play, Download, Clock, Database, Layers, Code2, Cpu, ChevronDown, Sparkles,
  Bookmark, Square, AlertCircle,
} from 'lucide-react';
import { EditorState } from '@codemirror/state';
import { EditorView, keymap, placeholder } from '@codemirror/view';
import { sql as sqlLang, PostgreSQL, MySQL, SQLite, MSSQL, StandardSQL, SQLConfig } from '@codemirror/lang-sql';
import { oneDark } from '@codemirror/theme-one-dark';
import { autocompletion } from '@codemirror/autocomplete';
import { ExplainVisualizerModal } from './ExplainVisualizerModal';
import { Tooltip } from './Tooltip';
import { AutocompleteData, QueryResultPayload } from '../services/tauriBridge';

export interface MultiQueryResult {
  id: string;
  sql: string;
  result?: QueryResultPayload;
  error?: string;
  status: 'pending' | 'running' | 'success' | 'error';
}

interface SqlEditorProps {
  initialSql?: string;
  onRunQuery: (sql: string) => void;
  onCancelQuery?: () => void;
  queryResult: QueryResultPayload | null;
  /** Multi-statement result sets (when present, preferred over queryResult) */
  multiResults?: MultiQueryResult[];
  isLoading: boolean;
  onSaveQuery: (name: string, sql: string) => void;
  /** Live schema map for CodeMirror autocomplete */
  schemaData?: AutocompleteData | null;
  dialectHint?: string;
  readOnlyConnection?: boolean;
  onProfileQuery?: (sql: string) => void;
}

export interface DialectOption {
  id: string;
  name: string;
  category: 'Relational' | 'NoSQL' | 'Cloud';
  dialectObj: any;
}

export const DIALECTS: DialectOption[] = [
  { id: 'postgres', name: 'PostgreSQL', category: 'Relational', dialectObj: PostgreSQL },
  { id: 'mysql', name: 'MySQL', category: 'Relational', dialectObj: MySQL },
  { id: 'mariadb', name: 'MariaDB', category: 'Relational', dialectObj: MySQL },
  { id: 'sqlite', name: 'SQLite', category: 'Relational', dialectObj: SQLite },
  { id: 'duckdb', name: 'DuckDB', category: 'Relational', dialectObj: SQLite },
  { id: 'turso', name: 'Turso (libSQL)', category: 'Relational', dialectObj: SQLite },
  { id: 'cockroachdb', name: 'CockroachDB', category: 'Relational', dialectObj: PostgreSQL },
  { id: 'redshift', name: 'Amazon Redshift', category: 'Cloud', dialectObj: PostgreSQL },
  { id: 'mssql', name: 'SQL Server (highlight only)', category: 'Relational', dialectObj: MSSQL },
  { id: 'standard', name: 'Standard SQL (highlight only)', category: 'Relational', dialectObj: StandardSQL },
];

export interface SqlSnippet {
  name: string;
  description: string;
  sql: string;
}

export const SQL_SNIPPETS: SqlSnippet[] = [
  {
    name: 'SELECT with JOIN & Filter',
    description: 'Fetch records with relational join and criteria',
    sql: 'SELECT u.id, u.email, u.name, a.action\nFROM users u\nJOIN audit_logs a ON u.id = a.id\nWHERE u.role = \'Backend Lead\'\nLIMIT 50;',
  },
  {
    name: 'Batch INSERT Into Table',
    description: 'Insert multiple rows in a single atomic statement',
    sql: 'INSERT INTO users (email, name, role)\nVALUES\n  (\'test1@devdash.io\', \'Test One\', \'Developer\'),\n  (\'test2@devdash.io\', \'Test Two\', \'Designer\');',
  },
  {
    name: 'CREATE TABLE with Constraints',
    description: 'DDL template for creating a new table with primary key',
    sql: 'CREATE TABLE IF NOT EXISTS products (\n  id SERIAL PRIMARY KEY,\n  title VARCHAR(255) NOT NULL,\n  price DECIMAL(10, 2) DEFAULT 0.00,\n  created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP\n);',
  },
  {
    name: 'CREATE Index',
    description: 'Speed up queries on specific columns',
    sql: 'CREATE INDEX IF NOT EXISTS idx_users_email ON users (email);',
  },
  {
    name: 'UPSERT (On Conflict Update)',
    description: 'Insert or update existing row on unique key conflict',
    sql: 'INSERT INTO users (id, email, name)\nVALUES (1, \'akshat@devdash.io\', \'Akshat\')\nON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email;',
  },
];

function buildSqlSchema(data?: AutocompleteData | null): SQLConfig['schema'] {
  if (!data?.table_columns?.length) return undefined;
  const schema: Record<string, string[]> = {};
  for (const t of data.table_columns) {
    schema[t.table_name] = t.columns;
  }
  return schema;
}

export const SqlEditor: React.FC<SqlEditorProps> = ({
  initialSql = '',
  onRunQuery,
  onCancelQuery,
  queryResult,
  multiResults,
  isLoading,
  onSaveQuery,
  schemaData,
  dialectHint,
  readOnlyConnection,
  onProfileQuery,
}) => {
  const [sql, setSql] = useState(initialSql || '');
  const [dialect, setDialect] = useState<string>(() => {
    const hint = (dialectHint || 'postgres').toLowerCase();
    if (DIALECTS.some((d) => d.id === hint)) return hint;
    if (hint === 'postgresql') return 'postgres';
    return 'postgres';
  });
  const [showDialectMenu, setShowDialectMenu] = useState(false);
  const [showSnippetsMenu, setShowSnippetsMenu] = useState(false);
  const [saveName, setSaveName] = useState('');
  const [showSaveInput, setShowSaveInput] = useState(false);
  const [isExplainOpen, setIsExplainOpen] = useState(false);
  const [activeResultIdx, setActiveResultIdx] = useState(0);
  const [editorHeightPercent, setEditorHeightPercent] = useState(35);

  const containerRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const isDraggingRef = useRef(false);
  const onRunRef = useRef(onRunQuery);
  onRunRef.current = onRunQuery;

  const selectedDialect = DIALECTS.find((d) => d.id === dialect) || DIALECTS[0];
  const cmSchema = useMemo(() => buildSqlSchema(schemaData), [schemaData]);

  // Initialize / re-init CodeMirror when dialect or schema changes
  useEffect(() => {
    if (!editorRef.current) return;

    const doc = viewRef.current?.state.doc.toString() ?? sql;

    const state = EditorState.create({
      doc,
      extensions: [
        oneDark,
        sqlLang({
          dialect: selectedDialect.dialectObj,
          schema: cmSchema,
          upperCaseKeywords: true,
        }),
        autocompletion({ activateOnTyping: true }),
        placeholder('Write SQL…  Cmd/Ctrl+Enter to run · schema autocomplete when connected'),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            setSql(update.state.doc.toString());
          }
        }),
        keymap.of([
          {
            key: 'Mod-Enter',
            run: () => {
              const text = viewRef.current?.state.doc.toString() || '';
              onRunRef.current(text);
              return true;
            },
          },
          {
            key: 'Mod-i',
            run: () => {
              formatSqlInView();
              return true;
            },
          },
          {
            key: 'Mod-s',
            run: () => {
              setShowSaveInput(true);
              return true;
            },
          },
        ]),
        EditorView.theme({
          '&': { height: '100%', fontSize: '13px' },
          '.cm-scroller': { overflow: 'auto', fontFamily: "'JetBrains Mono', monospace" },
          '.cm-content': { padding: '8px 0' },
        }),
      ],
    });

    if (viewRef.current) {
      viewRef.current.destroy();
    }
    const view = new EditorView({
      state,
      parent: editorRef.current,
    });
    viewRef.current = view;

    return () => {
      view.destroy();
      viewRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dialect, cmSchema]);

  useEffect(() => {
    if (dialectHint) {
      const hint = dialectHint.toLowerCase() === 'postgresql' ? 'postgres' : dialectHint.toLowerCase();
      if (DIALECTS.some((d) => d.id === hint)) setDialect(hint);
    }
  }, [dialectHint]);

  // Reset result tab when multi results change
  useEffect(() => {
    setActiveResultIdx(0);
  }, [multiResults]);

  const formatSqlInView = useCallback(() => {
    const currentSql = viewRef.current ? viewRef.current.state.doc.toString() : sql;
    const keywords = [
      'SELECT', 'FROM', 'WHERE', 'AND', 'OR', 'LEFT JOIN', 'RIGHT JOIN', 'INNER JOIN',
      'JOIN', 'GROUP BY', 'ORDER BY', 'LIMIT', 'HAVING', 'INSERT INTO', 'VALUES',
      'UPDATE', 'SET', 'DELETE FROM', 'CREATE TABLE', 'ALTER TABLE', 'DROP TABLE',
      'RETURNING', 'WITH', 'UNION', 'INTERSECT', 'EXCEPT',
    ];

    let formatted = currentSql;
    keywords.forEach((kw) => {
      const regex = new RegExp(`\\b${kw}\\b`, 'gi');
      formatted = formatted.replace(regex, `\n${kw}`);
    });
    formatted = formatted
      .split('\n')
      .map((l) => l.trim())
      .filter(Boolean)
      .join('\n')
      .trim();

    if (viewRef.current) {
      viewRef.current.dispatch({
        changes: { from: 0, to: viewRef.current.state.doc.length, insert: formatted },
      });
    }
    setSql(formatted);
  }, [sql]);

  const handleRunAction = () => {
    if (readOnlyConnection) {
      const text = (viewRef.current?.state.doc.toString() || sql).trim().toUpperCase();
      const write =
        text.startsWith('INSERT') ||
        text.startsWith('UPDATE') ||
        text.startsWith('DELETE') ||
        text.startsWith('DROP') ||
        text.startsWith('ALTER') ||
        text.startsWith('TRUNCATE') ||
        text.startsWith('CREATE') ||
        text.startsWith('GRANT') ||
        text.startsWith('REVOKE');
      if (write) {
        alert('This connection is read-only. Write/DDL statements are blocked.');
        return;
      }
    }
    onRunQuery(viewRef.current?.state.doc.toString() || sql);
  };

  const exportToJson = () => {
    const active = displayResult;
    if (!active) return;
    const objs = active.rows.map((row) => {
      const o: Record<string, unknown> = {};
      active.columns.forEach((c, i) => {
        o[c.name] = row[i];
      });
      return o;
    });
    const blob = new Blob([JSON.stringify(objs, null, 2)], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `query_results_${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const handleMouseDown = (e: React.MouseEvent) => {
    e.preventDefault();
    isDraggingRef.current = true;
    const handleMouseMove = (ev: MouseEvent) => {
      if (!isDraggingRef.current || !containerRef.current) return;
      const containerRect = containerRef.current.getBoundingClientRect();
      const newPercent = ((ev.clientY - containerRect.top) / containerRect.height) * 100;
      if (newPercent >= 15 && newPercent <= 80) {
        setEditorHeightPercent(newPercent);
      }
    };
    const handleMouseUp = () => {
      isDraggingRef.current = false;
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  };

  const multi = multiResults && multiResults.length > 0 ? multiResults : null;
  const displayResult: QueryResultPayload | null = multi
    ? multi[activeResultIdx]?.result ?? null
    : queryResult;
  const displayError = multi ? multi[activeResultIdx]?.error : undefined;

  const isSelectQuery = sql.trim().toUpperCase().startsWith('SELECT') || sql.trim().toUpperCase().startsWith('WITH');

  return (
    <div ref={containerRef} className="flex flex-col h-full bg-base text-text font-sans relative select-none">
      <div className="h-11 bg-surface border-b border-border px-4 flex items-center justify-between shrink-0">
        <div className="flex items-center space-x-3">
          <div className="flex items-center space-x-2">
            <button
              onClick={handleRunAction}
              disabled={isLoading}
              className="px-[14px] py-[6px] rounded-[6px] bg-accent hover:bg-accentHover text-white font-medium text-[13px] flex items-center space-x-1.5 shadow-md shadow-accent/20 transition-all disabled:opacity-50 font-sans outline-none focus-visible:ring-2 focus-visible:ring-accent/50"
            >
              {isLoading ? (
                <svg className="animate-spin h-3.5 w-3.5 text-white" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
                </svg>
              ) : (
                <Play className="w-3.5 h-3.5 fill-current" />
              )}
              <span>{isLoading ? 'Running…' : 'Run Query'}</span>
            </button>
            {isLoading && onCancelQuery && (
              <button
                onClick={onCancelQuery}
                className="px-2.5 py-1.5 rounded border border-rose-500/40 bg-rose-950/40 text-rose-300 text-xs font-semibold flex items-center space-x-1 hover:bg-rose-900/40"
                title="Cancel running query"
              >
                <Square className="w-3 h-3 fill-current" />
                <span>Cancel</span>
              </button>
            )}
            <span className="text-[11px] text-textMuted font-sans">Cmd+Enter</span>
            {readOnlyConnection && (
              <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/30 font-semibold">
                READ-ONLY
              </span>
            )}
          </div>

          <Tooltip content={isSelectQuery ? 'Visualize SQL Execution Plan' : 'Explain Plan only available for SELECT queries.'}>
            <button
              onClick={() => isSelectQuery && setIsExplainOpen(true)}
              disabled={!isSelectQuery}
              className={`px-[14px] py-[6px] rounded-[6px] border border-[rgba(255,255,255,0.12)] bg-transparent text-accent text-[13px] font-semibold transition-all flex items-center space-x-1.5 font-sans ${
                isSelectQuery ? 'hover:bg-[rgba(255,255,255,0.06)]' : 'opacity-40 cursor-not-allowed'
              }`}
            >
              <Cpu className="w-3.5 h-3.5 text-accent" />
              <span>Explain Plan</span>
            </button>
          </Tooltip>

          {onProfileQuery && (
            <Tooltip content="Profile with EXPLAIN / EXPLAIN ANALYZE">
              <button
                onClick={() =>
                  onProfileQuery(viewRef.current?.state.doc.toString() || sql)
                }
                className="px-2.5 py-1.5 rounded border border-[rgba(255,255,255,0.12)] bg-transparent hover:bg-[rgba(255,255,255,0.06)] text-text text-xs font-semibold flex items-center space-x-1"
              >
                <Cpu className="w-3.5 h-3.5 text-warning" />
                <span>Profile</span>
              </button>
            </Tooltip>
          )}

          <div className="relative">
            <button
              onClick={() => setShowDialectMenu(!showDialectMenu)}
              className="px-2.5 py-1 rounded bg-base hover:bg-surface2 border border-border text-xs text-text flex items-center space-x-1.5 transition-colors font-sans"
            >
              <Code2 className="w-3.5 h-3.5 text-accent" />
              <span className="font-medium text-[12px]">{selectedDialect.name}</span>
              <ChevronDown className="w-3.5 h-3.5 text-textMuted ml-0.5" />
            </button>
            {showDialectMenu && (
              <div className="absolute left-0 mt-1 w-56 bg-surface border border-border rounded-lg shadow-2xl py-1 z-50 max-h-72 overflow-y-auto">
                {DIALECTS.map((d) => (
                  <button
                    key={d.id}
                    onClick={() => {
                      setDialect(d.id);
                      setShowDialectMenu(false);
                    }}
                    className={`w-full text-left px-3 py-1.5 text-xs hover:bg-surface2 flex items-center justify-between ${
                      d.id === dialect ? 'bg-accent/15 text-accent font-medium' : 'text-text'
                    }`}
                  >
                    <span>{d.name}</span>
                    <span className="text-[10px] text-textMuted font-mono">{d.category}</span>
                  </button>
                ))}
              </div>
            )}
          </div>

          <Tooltip content="Beautify / Format SQL (Cmd+I)">
            <button
              onClick={formatSqlInView}
              className="px-2.5 py-1.5 rounded border border-[rgba(255,255,255,0.12)] bg-transparent hover:bg-[rgba(255,255,255,0.06)] text-text text-xs font-semibold flex items-center space-x-1"
            >
              <Sparkles className="w-3.5 h-3.5 text-warning" />
              <span>Format</span>
            </button>
          </Tooltip>

          <div className="relative">
            <button
              onClick={() => setShowSnippetsMenu(!showSnippetsMenu)}
              className="px-2.5 py-1.5 rounded border border-[rgba(255,255,255,0.12)] bg-transparent hover:bg-[rgba(255,255,255,0.06)] text-text text-xs font-semibold flex items-center space-x-1"
            >
              <Bookmark className="w-3.5 h-3.5 text-accent" />
              <span>Snippets</span>
            </button>
            {showSnippetsMenu && (
              <div className="absolute left-0 mt-1 w-64 bg-surface border border-border rounded-lg shadow-2xl py-1 z-50">
                {SQL_SNIPPETS.map((snippet) => (
                  <button
                    key={snippet.name}
                    onClick={() => {
                      if (viewRef.current) {
                        viewRef.current.dispatch({
                          changes: {
                            from: 0,
                            to: viewRef.current.state.doc.length,
                            insert: snippet.sql,
                          },
                        });
                      }
                      setSql(snippet.sql);
                      setShowSnippetsMenu(false);
                    }}
                    className="w-full text-left px-3 py-2 hover:bg-surface2 border-b border-border/20 last:border-0"
                  >
                    <div className="text-xs font-semibold text-text">{snippet.name}</div>
                    <div className="text-[10px] text-textMuted truncate">{snippet.description}</div>
                  </button>
                ))}
              </div>
            )}
          </div>

          {showSaveInput ? (
            <div className="flex items-center space-x-1.5">
              <input
                type="text"
                placeholder="Query name..."
                value={saveName}
                onChange={(e) => setSaveName(e.target.value)}
                className="bg-base border border-border rounded px-2 py-1 text-xs text-text outline-none focus-visible:ring-2 focus-visible:ring-accent/50"
                autoFocus
              />
              <button
                onClick={() => {
                  if (saveName.trim()) {
                    onSaveQuery(saveName, viewRef.current?.state.doc.toString() || sql);
                    setShowSaveInput(false);
                    setSaveName('');
                  }
                }}
                className="px-2.5 py-1 rounded bg-success text-white text-xs"
              >
                Save
              </button>
            </div>
          ) : (
            <button
              onClick={() => setShowSaveInput(true)}
              className="px-3 py-1.5 rounded border border-[rgba(255,255,255,0.12)] bg-transparent hover:bg-[rgba(255,255,255,0.06)] text-text text-xs font-semibold"
            >
              Save Query
            </button>
          )}

          {schemaData && (
            <span className="text-[10px] text-textMuted font-mono" title="Schema loaded for autocomplete">
              {schemaData.tables.length} tables · {schemaData.fetch_time_ms.toFixed(0)}ms schema
            </span>
          )}
        </div>

        {displayResult && (
          <div className="flex items-center space-x-4 text-xs font-mono text-textMuted">
            <div className="flex items-center space-x-1 text-accent">
              <Clock className="w-3.5 h-3.5" />
              <span>{displayResult.execution_time_ms} ms</span>
            </div>
            <div className="flex items-center space-x-1 text-accentHover">
              <Layers className="w-3.5 h-3.5" />
              <span>
                {displayResult.rows?.length ?? 0} row(s)
                {displayResult.affected_rows && displayResult.rows?.length === 0
                  ? ` · ${displayResult.affected_rows} affected`
                  : ''}
              </span>
            </div>
            <button
              onClick={exportToJson}
              className="p-1.5 rounded border border-[rgba(255,255,255,0.12)] hover:bg-[rgba(255,255,255,0.06)] text-text"
              title="Export as JSON"
            >
              <Download className="w-3.5 h-3.5" />
            </button>
          </div>
        )}
      </div>

      <div
        style={{ height: `calc(${editorHeightPercent}% - 44px)` }}
        className="bg-base border-b border-border text-[13px] font-mono overflow-hidden min-h-[80px]"
        ref={editorRef}
      />

      <div
        onMouseDown={handleMouseDown}
        className="h-1.5 bg-transparent border-t border-b border-border/40 hover:bg-accent/40 cursor-row-resize shrink-0 transition-colors relative z-20"
        title="Drag to resize editor"
      >
        <div className="absolute inset-x-0 top-1/2 h-[1px] bg-border/60" />
      </div>

      {/* Multi-statement result tabs */}
      {multi && multi.length > 1 && (
        <div className="h-8 bg-surface border-b border-border px-2 flex items-center space-x-1 overflow-x-auto shrink-0">
          {multi.map((r, idx) => (
            <button
              key={r.id}
              onClick={() => setActiveResultIdx(idx)}
              className={`px-2.5 py-1 rounded text-[11px] font-mono truncate max-w-[180px] ${
                idx === activeResultIdx
                  ? 'bg-accent/20 text-accent'
                  : 'text-textMuted hover:text-text hover:bg-surface2'
              }`}
              title={r.sql}
            >
              {r.status === 'error' ? '⚠ ' : ''}
              Result {idx + 1}
              {r.result ? ` (${r.result.rows?.length ?? 0})` : r.status === 'running' ? ' …' : ''}
            </button>
          ))}
        </div>
      )}

      <div style={{ height: `${100 - editorHeightPercent}%` }} className="overflow-auto bg-base min-h-[100px]">
        {isLoading && !displayResult ? (
          <div className="flex items-center justify-center h-full text-textMuted font-sans text-[13px]">
            Executing query against server…
          </div>
        ) : displayError ? (
          <div className="flex items-start space-x-2 p-4 text-rose-300 text-xs font-mono">
            <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            <pre className="whitespace-pre-wrap">{displayError}</pre>
          </div>
        ) : !displayResult ? (
          <div className="flex items-center justify-center h-full text-textMuted/45 font-sans text-[13px] flex-col space-y-2 select-none min-h-[150px]">
            <Database className="w-8 h-8 opacity-30" />
            <span>Click Run Query or press Cmd+Enter · multi-statement scripts produce one tab per result</span>
          </div>
        ) : (displayResult.rows?.length ?? 0) === 0 ? (
          <div className="flex items-center justify-center h-full text-textMuted/45 font-sans text-[13px]">
            Query executed successfully.
            {displayResult.affected_rows
              ? ` ${displayResult.affected_rows} row(s) affected.`
              : ' 0 rows returned.'}
          </div>
        ) : (
          <table className="w-full border-collapse text-left font-mono text-[13px]">
            <thead className="sticky top-0 bg-surface border-b border-border shadow-sm z-10">
              <tr>
                <th className="w-10 px-2 py-1.5 text-center text-textMuted border-r border-border font-sans text-[11px] uppercase tracking-[0.06em] opacity-60">
                  #
                </th>
                {displayResult.columns.map((col, idx) => (
                  <th
                    key={idx}
                    className="px-3 py-1.5 text-text border-r border-border whitespace-nowrap bg-surface font-sans text-[11px] uppercase tracking-[0.06em]"
                  >
                    <span className="text-text/60 font-semibold">{col.name}</span>
                    <span className="text-[11px] text-text/45 font-sans font-normal ml-1">
                      {col.type_name}
                    </span>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {displayResult.rows.map((row, rowIdx) => (
                <tr
                  key={rowIdx}
                  className="hover:bg-[rgba(255,255,255,0.04)] transition-colors border-b border-border/40 even:bg-surface/20"
                >
                  <td className="w-10 px-2 py-1 text-center text-textMuted border-r border-border/40 font-mono text-[11px]">
                    {rowIdx + 1}
                  </td>
                  {row.map((val: any, colIdx: number) => (
                    <td
                      key={colIdx}
                      className="px-3 py-1 border-r border-border/40 whitespace-nowrap text-text font-mono text-[13px] max-w-xs truncate"
                    >
                      {val === null ? (
                        <span className="text-textMuted italic font-sans text-xs">NULL</span>
                      ) : (
                        String(val)
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      <ExplainVisualizerModal isOpen={isExplainOpen} onClose={() => setIsExplainOpen(false)} sql={sql} />
    </div>
  );
};
