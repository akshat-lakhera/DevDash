import React, { useState, useEffect } from 'react';
import { ConnectionConfig, ConnectionEnvironment, DbKind } from '../types';
import { X, Server, Shield, KeyRound, Network, AlertTriangle } from 'lucide-react';
import { testDbConnection, testSshTunnel, TestConnectionResultPayload } from '../services/tauriBridge';
import {
  CONNECTION_ENVIRONMENTS,
  resolveReadOnlyFlag,
} from '../utils/connectionEnv';

interface ConnectionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSave: (conn: Omit<ConnectionConfig, 'id'>, password: string) => void;
  initialDbKind?: DbKind;
}

export const ConnectionModal: React.FC<ConnectionModalProps> = ({
  isOpen,
  onClose,
  onSave,
  initialDbKind,
}) => {
  const [activeTab, setActiveTab] = useState<'general' | 'ssh'>('general');
  const [name, setName] = useState('');
  const [dbType, setDbType] = useState<DbKind>('postgres');
  const [host, setHost] = useState('localhost');
  const [port, setPort] = useState(5432);
  const [user, setUser] = useState('postgres');
  const [password, setPassword] = useState('');
  const [database, setDatabase] = useState('postgres');
  const [environment, setEnvironment] = useState<ConnectionEnvironment>('dev');
  const [isReadOnly, setIsReadOnly] = useState(false);
  const [rawUrl, setRawUrl] = useState('');
  const [allowWritesOnProd, setAllowWritesOnProd] = useState(false);

  // SSH Tunnel State
  const [sshEnabled, setSshEnabled] = useState(false);
  const [sshHost, setSshHost] = useState('');
  const [sshPort, setSshPort] = useState(22);
  const [sshUser, setSshUser] = useState('');
  const [sshKeyPath, setSshKeyPath] = useState('~/.ssh/id_rsa');

  // Test Connection Diagnostics State
  const [isTesting, setIsTesting] = useState(false);
  const [testStatus, setTestStatus] = useState<TestConnectionResultPayload | null>(null);

  const handleTestConnection = async () => {
    if (dbType === 'turso') {
      if (!host.trim() || host === 'localhost') {
        setTestStatus({
          success: false,
          latency_ms: 0,
          message: 'Turso connection requires a database host URL (e.g., libsql://test-sasuke.aws-ap-south-1.turso.io).',
        });
        return;
      }
      if (!password.trim()) {
        setTestStatus({
          success: false,
          latency_ms: 0,
          message: 'Turso connection requires an Auth Token / Password. Please paste your Turso JWT Token.',
        });
        return;
      }
    }

    setIsTesting(true);
    setTestStatus(null);

    if (activeTab === 'ssh' || sshEnabled) {
      const sshRes = await testSshTunnel({
        enabled: true,
        host: sshHost,
        port: Number(sshPort),
        user: sshUser,
        key_path: sshKeyPath,
        password: password || undefined,
      });
      if (!sshRes.success) {
        setTestStatus(sshRes);
        setIsTesting(false);
        return;
      }
    }

    const res = await testDbConnection(
      {
        db_type: dbType,
        host,
        port: Number(port),
        user,
        database,
      },
      password
    );
    setTestStatus(res);
    setIsTesting(false);
  };

  const [sslMode, setSslMode] = useState<'prefer' | 'require' | 'verify-full'>('require');

  const handleDriverChange = (kind: DbKind) => {
    setDbType(kind);
    switch (kind) {
      case 'postgres': setPort(5432); setUser('postgres'); setDatabase('neondb'); break;
      case 'mysql': setPort(3306); setUser('root'); setDatabase('mysql'); break;
      case 'mariadb': setPort(3306); setUser('root'); setDatabase('mysql'); break;
      case 'sqlite': setPort(0); setUser(''); setDatabase('./database.sqlite'); break;
      case 'cockroachdb': setPort(26257); setUser('root'); setDatabase('defaultdb'); break;
      case 'redshift': setPort(5439); setUser('awsuser'); setDatabase('dev'); break;
      case 'duckdb': setPort(0); setUser(''); setDatabase('./analytics.duckdb'); break;
      case 'turso': setPort(0); setUser(''); setDatabase('main'); setHost(prev => prev === 'localhost' ? '' : prev); break;
      case 'redis': setPort(6379); setUser(''); setDatabase('0'); break;
      case 'mssql': setPort(1433); setUser('sa'); setDatabase('master'); break;
      case 'oracle': setPort(1521); setUser('system'); setDatabase('ORCL'); break;
      case 'snowflake': setPort(443); setUser('admin'); setDatabase('DEMO_DB'); break;
      case 'mongodb': setPort(27017); setUser('admin'); setDatabase('test'); break;
      case 'cassandra': setPort(9042); setUser('cassandra'); setDatabase('system'); break;
      case 'clickhouse': setPort(8123); setUser('default'); setDatabase('default'); break;
      case 'bigquery': setPort(0); setUser(''); setDatabase('my-project-id'); break;
    }
  };

  useEffect(() => {
    if (isOpen) {
      handleDriverChange(initialDbKind || 'postgres');
      setName('');
      setEnvironment('dev');
      setIsReadOnly(false);
      setAllowWritesOnProd(false);
    }
  }, [isOpen, initialDbKind]);

  // Production defaults to read-only; flipping env to prod turns RO on.
  useEffect(() => {
    if (environment === 'prod' && !allowWritesOnProd) {
      setIsReadOnly(true);
    }
  }, [environment, allowWritesOnProd]);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (dbType === 'turso') {
      if (!host.trim() || host === 'localhost') {
        alert('Turso connection requires a database host URL (e.g. libsql://test-sasuke.aws-ap-south-1.turso.io).');
        return;
      }
      if (!password.trim()) {
        alert('Turso connection requires an Auth Token / Password. Please paste your Turso JWT Token.');
        return;
      }
    }
    const draft: Omit<ConnectionConfig, 'id'> = {
      name: name || `${dbType.toUpperCase()} Connection`,
      db_type: dbType,
      host,
      port: Number(port),
      user,
      database,
      ssl_mode: sslMode,
      environment,
      allow_writes_on_prod: environment === 'prod' ? allowWritesOnProd : false,
      is_read_only: resolveReadOnlyFlag({
        environment,
        is_read_only: isReadOnly,
        allow_writes_on_prod: environment === 'prod' ? allowWritesOnProd : false,
      }),
      ssh_config: sshEnabled
        ? {
            enabled: true,
            host: sshHost,
            port: Number(sshPort),
            user: sshUser,
            key_path: sshKeyPath,
          }
        : undefined,
    };
    onSave(draft, password);
    onClose();
  };

  const isFileBased = dbType === 'sqlite' || dbType === 'duckdb';

  const parseConnectionString = (urlStr: string) => {
    setRawUrl(urlStr);
    if (!urlStr.trim()) return;
    try {
      let cleanStr = urlStr.trim();
      let isTursoUrl = false;
      if (cleanStr.startsWith('libsql://') || cleanStr.includes('.turso.io')) {
        isTursoUrl = true;
        if (cleanStr.startsWith('libsql://')) {
          cleanStr = cleanStr.replace('libsql://', 'https://');
        }
      }

      const parsed = new URL(cleanStr);
      if (isTursoUrl || parsed.hostname.includes('.turso.io')) {
        setDbType('turso');
        setPort(0);
        const token = parsed.searchParams.get('authToken') || parsed.searchParams.get('jwt') || parsed.password;
        if (token) setPassword(decodeURIComponent(token));
        const hostName = parsed.hostname;
        setHost(hostName);
        if (parsed.username) {
          setUser(decodeURIComponent(parsed.username));
        } else {
          const parts = hostName.split('-');
          if (parts.length > 1) setUser(parts[1].split('.')[0]);
        }
        setDatabase('main');
        const firstSeg = hostName.split('.')[0];
        setName(firstSeg ? firstSeg.charAt(0).toUpperCase() + firstSeg.slice(1) : 'Turso DB');
        return;
      }

      if (parsed.protocol.startsWith('postgres') || parsed.protocol.startsWith('postgresql')) {
        setDbType('postgres');
      } else if (parsed.protocol.startsWith('mysql')) {
        setDbType('mysql');
      } else if (parsed.protocol.startsWith('redis')) {
        setDbType('redis');
      }
      if (parsed.hostname) setHost(parsed.hostname);
      if (parsed.port) {
        setPort(Number(parsed.port));
      } else if (parsed.protocol.startsWith('postgres') || parsed.protocol.startsWith('postgresql')) {
        setPort(5432);
      } else if (parsed.protocol.startsWith('mysql')) {
        setPort(3306);
      } else if (parsed.protocol.startsWith('redis')) {
        setPort(6379);
      }
      if (parsed.username) setUser(decodeURIComponent(parsed.username));
      if (parsed.password) setPassword(decodeURIComponent(parsed.password));
      if (parsed.pathname && parsed.pathname.length > 1) {
        setDatabase(decodeURIComponent(parsed.pathname.substring(1)));
      }
      const ssl = parsed.searchParams.get('sslmode');
      if (ssl === 'require' || ssl === 'prefer' || ssl === 'verify-full') {
        setSslMode(ssl);
      }
      if (parsed.hostname) {
        const firstSeg = parsed.hostname.split('.')[0];
        setName(firstSeg ? firstSeg.charAt(0).toUpperCase() + firstSeg.slice(1) : 'Database');
      }
    } catch {
      // Ignore invalid URL formatting while user is typing
    }
  };

  return (
    <div className="fixed inset-0 bg-[#0F0F10]/80 backdrop-blur-md flex items-center justify-center z-50 animate-fadeIn">
      <div className="w-full max-w-lg bg-[#141416] border border-white/10 rounded-xl shadow-2xl overflow-hidden text-text font-sans">
        {/* Header */}
        <div className="p-4 bg-[#1A1A1C] border-b border-white/10 flex items-center justify-between">
          <div className="flex items-center space-x-2">
            <Server className="w-5 h-5 text-accent" />
            <h2 className="font-semibold text-sm text-text">Add New Connection</h2>
          </div>
          <button onClick={onClose} className="p-1 rounded text-textMuted hover:text-text transition-colors">
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Tab Switcher Bar */}
        <div className="flex border-b border-white/10 bg-base px-4 text-xs">
          <button
            type="button"
            onClick={() => setActiveTab('general')}
            className={`py-2 px-3 border-b-2 font-medium transition-colors flex items-center space-x-1.5 ${
              activeTab === 'general'
                ? 'border-accent text-accent'
                : 'border-transparent text-textMuted hover:text-text'
            }`}
          >
            <Server className="w-3.5 h-3.5" />
            <span>General Config</span>
          </button>
          <button
            type="button"
            onClick={() => setActiveTab('ssh')}
            className={`py-2 px-3 border-b-2 font-medium transition-colors flex items-center space-x-1.5 ${
              activeTab === 'ssh'
                ? 'border-accent text-accent'
                : 'border-transparent text-textMuted hover:text-text'
            }`}
          >
            <Network className="w-3.5 h-3.5" />
            <span>SSH Tunnel {sshEnabled && <span className="w-1.5 h-1.5 rounded-full bg-success inline-block ml-1" />}</span>
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-4 space-y-3 text-xs">
          {activeTab === 'general' ? (
            <>
              {/* Quick Paste Connection String Box */}
              <div className="p-2.5 bg-[#0F0F10] border border-accent/30 rounded-lg space-y-1">
                <label className="block text-[11px] font-semibold text-accent flex items-center justify-between">
                  <span>⚡ Paste Connection String / URL (Neon, Supabase, RDS)</span>
                </label>
                <input
                  type="text"
                  placeholder="e.g. postgres://user:pass@ep-cool-123.neon.tech/neondb?sslmode=require"
                  value={rawUrl}
                  onChange={(e) => parseConnectionString(e.target.value)}
                  className="w-full bg-[#141416] border border-white/10 rounded px-2.5 py-1 text-text placeholder-textMuted/40 text-[11px] outline-none focus:border-accent"
                />
              </div>

              <div>
                <label className="block font-medium text-textMuted mb-1">Connection Name</label>
                <input
                  type="text"
                  placeholder="e.g. Production Database"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text placeholder-textMuted/50 outline-none focus:ring-2 focus:ring-accent/50"
                />
              </div>

              <div>
                <label className="block font-medium text-textMuted mb-1">Database Driver</label>
                <select
                  value={dbType}
                  onChange={(e) => handleDriverChange(e.target.value as DbKind)}
                  className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50 cursor-pointer"
                >
                  <optgroup label="Supported (native compiled drivers)">
                    <option value="postgres">PostgreSQL (Neon / Supabase / RDS)</option>
                    <option value="mysql">MySQL</option>
                    <option value="mariadb">MariaDB</option>
                    <option value="sqlite">SQLite</option>
                    <option value="duckdb">DuckDB (file / :memory:)</option>
                    <option value="cockroachdb">CockroachDB (Postgres wire)</option>
                    <option value="redshift">Amazon Redshift (Postgres wire)</option>
                    <option value="turso">Turso (SQLite engine)</option>
                    <option value="redis">Redis (RESP Protocol)</option>
                  </optgroup>
                </select>
                <p className="mt-1 text-[10px] text-textMuted">
                  For DuckDB, set Database to a .duckdb path or :memory:. Oracle, Snowflake, and BigQuery remain UI stubs.
                </p>
              </div>

              {!isFileBased && (
                <>
                  <div className="grid grid-cols-3 gap-2">
                    <div className="col-span-2">
                      <label className="block font-medium text-textMuted mb-1">Host</label>
                      <input
                        type="text"
                        value={host}
                        onChange={(e) => setHost(e.target.value)}
                        className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50"
                      />
                    </div>
                    <div>
                      <label className="block font-medium text-textMuted mb-1">Port</label>
                      <input
                        type="number"
                        value={port}
                        onChange={(e) => setPort(Number(e.target.value))}
                        className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50"
                      />
                    </div>
                  </div>

                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block font-medium text-textMuted mb-1">User</label>
                      <input
                        type="text"
                        value={user}
                        onChange={(e) => setUser(e.target.value)}
                        className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50"
                      />
                    </div>
                    <div>
                      <label className="block font-medium text-textMuted mb-1">
                        {dbType === 'turso' ? 'Auth Token / Password' : 'Password'}
                      </label>
                      <input
                        type="password"
                        placeholder={dbType === 'turso' ? 'Paste Turso JWT Token' : 'OS Keyring Protected'}
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text placeholder-textMuted/50 outline-none focus:ring-2 focus:ring-accent/50"
                      />
                    </div>
                  </div>
                </>
              )}

              <div>
                <label className="block font-medium text-textMuted mb-1">
                  {isFileBased ? 'File Path' : 'Database Name'}
                </label>
                <input
                  type="text"
                  value={database}
                  onChange={(e) => setDatabase(e.target.value)}
                  className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50"
                />
              </div>

              {/* Environment */}
              <div>
                <label className="block font-medium text-textMuted mb-1.5">Environment</label>
                <div className="grid grid-cols-4 gap-1.5">
                  {CONNECTION_ENVIRONMENTS.map((env) => {
                    const active = environment === env.id;
                    return (
                      <button
                        key={env.id}
                        type="button"
                        onClick={() => setEnvironment(env.id)}
                        className={`px-2 py-1.5 rounded-lg border text-[11px] font-semibold transition-colors ${
                          active
                            ? env.badgeClass + ' border'
                            : 'border-white/10 text-textMuted hover:border-white/20 bg-base'
                        }`}
                        title={env.description}
                      >
                        {env.short}
                      </button>
                    );
                  })}
                </div>
                <p className="mt-1 text-[10px] text-textMuted">
                  {CONNECTION_ENVIRONMENTS.find((e) => e.id === environment)?.description}
                </p>
              </div>

              {/* Read-Only Protection Toggle */}
              <div className="p-2.5 bg-base border border-white/10 rounded-lg flex items-center justify-between">
                <div className="flex items-center space-x-2">
                  <Shield
                    className={`w-4 h-4 ${
                      isReadOnly || (environment === 'prod' && !allowWritesOnProd)
                        ? 'text-warning'
                        : 'text-textMuted'
                    }`}
                  />
                  <div>
                    <div className="font-semibold text-text text-[12px]">Read-Only Mode</div>
                    <div className="text-[10px] text-textMuted">
                      Blocks DDL, INSERT, UPDATE, DELETE (enforced in UI + Rust backend)
                    </div>
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={
                    environment === 'prod' && !allowWritesOnProd ? true : isReadOnly
                  }
                  disabled={environment === 'prod' && !allowWritesOnProd}
                  onChange={(e) => setIsReadOnly(e.target.checked)}
                  className="w-4 h-4 accent-accent rounded cursor-pointer disabled:opacity-50"
                />
              </div>

              {environment === 'prod' && (
                <div className="p-2.5 bg-rose-500/10 border border-rose-500/30 rounded-lg space-y-2">
                  <div className="flex items-start gap-2 text-[11px] text-rose-300">
                    <AlertTriangle className="w-4 h-4 shrink-0 mt-0.5" />
                    <span>
                      Production connections are <strong>read-only by default</strong>. Only enable
                      writes if you accept the risk of mutating live data.
                    </span>
                  </div>
                  <label className="flex items-center gap-2 text-[11px] text-rose-200/90 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={allowWritesOnProd}
                      onChange={(e) => {
                        setAllowWritesOnProd(e.target.checked);
                        if (e.target.checked) setIsReadOnly(false);
                        else setIsReadOnly(true);
                      }}
                      className="w-3.5 h-3.5 accent-rose-500"
                    />
                    Allow writes on production (dangerous)
                  </label>
                </div>
              )}
            </>
          ) : (
            /* SSH Tunneling Tab */
            <div className="space-y-3">
              <div className="p-2.5 bg-base border border-white/10 rounded-lg flex items-center justify-between">
                <div className="flex items-center space-x-2">
                  <KeyRound className="w-4 h-4 text-accent" />
                  <div>
                    <div className="font-semibold text-text text-[12px]">Enable SSH Tunneling</div>
                    <div className="text-[10px] text-textMuted">Route connection securely through remote SSH bastion host</div>
                  </div>
                </div>
                <input
                  type="checkbox"
                  checked={sshEnabled}
                  onChange={(e) => setSshEnabled(e.target.checked)}
                  className="w-4 h-4 accent-accent rounded cursor-pointer"
                />
              </div>

              {sshEnabled && (
                <>
                  <div className="grid grid-cols-3 gap-2">
                    <div className="col-span-2">
                      <label className="block font-medium text-textMuted mb-1">SSH Host</label>
                      <input
                        type="text"
                        placeholder="bastion.example.com"
                        value={sshHost}
                        onChange={(e) => setSshHost(e.target.value)}
                        className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50"
                      />
                    </div>
                    <div>
                      <label className="block font-medium text-textMuted mb-1">SSH Port</label>
                      <input
                        type="number"
                        value={sshPort}
                        onChange={(e) => setSshPort(Number(e.target.value))}
                        className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50"
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block font-medium text-textMuted mb-1">SSH User</label>
                    <input
                      type="text"
                      placeholder="ubuntu / ec2-user"
                      value={sshUser}
                      onChange={(e) => setSshUser(e.target.value)}
                      className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50"
                    />
                  </div>

                  <div>
                    <label className="block font-medium text-textMuted mb-1">SSH Identity Key File Path</label>
                    <input
                      type="text"
                      placeholder="~/.ssh/id_rsa"
                      value={sshKeyPath}
                      onChange={(e) => setSshKeyPath(e.target.value)}
                      className="w-full bg-[#0F0F10] border border-white/10 rounded px-3 py-1.5 text-text outline-none focus:ring-2 focus:ring-accent/50 font-mono text-[11px]"
                    />
                  </div>
                </>
              )}
            </div>
          )}

          {testStatus && (
            <div className={`p-2.5 rounded text-[11px] flex items-center justify-between border ${
              testStatus.success ? 'bg-success/10 border-success/30 text-success' : 'bg-danger/10 border-danger/30 text-danger'
            }`}>
              <div className="flex items-center space-x-1.5">
                <span className="font-semibold">{testStatus.success ? '✓ Connected' : '✕ Connection Failed'}</span>
                <span>— {testStatus.message}</span>
              </div>
              {testStatus.latency_ms > 0 && (
                <span className="font-mono text-[10px] bg-black/30 px-1.5 py-0.5 rounded text-white/70">
                  {testStatus.latency_ms}ms
                </span>
              )}
            </div>
          )}

          <div className="pt-3 flex justify-between items-center border-t border-white/10">
            <button
              type="button"
              disabled={isTesting}
              onClick={handleTestConnection}
              className="px-3 py-1.5 rounded border border-white/15 bg-white/5 text-text hover:bg-white/10 transition-all font-medium flex items-center space-x-1.5 disabled:opacity-50"
            >
              {isTesting ? (
                <>
                  <span className="w-3 h-3 border-2 border-accent border-t-transparent rounded-full animate-spin" />
                  <span>Testing...</span>
                </>
              ) : (
                <span>Test Connection</span>
              )}
            </button>

            <div className="flex space-x-2">
              <button
                type="button"
                onClick={onClose}
                className="px-3 py-1.5 rounded bg-surface2 text-text hover:bg-surface2/80 transition-all font-medium"
              >
                Cancel
              </button>
              <button
                type="submit"
                className="px-4 py-1.5 rounded bg-accent text-white font-medium hover:bg-accentHover shadow-md shadow-accent/20 transition-all"
              >
                Save Connection
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
};
