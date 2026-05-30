import { useState, useEffect, useRef, useCallback, useMemo, type ReactNode } from 'react';
import { 
  Play, Search, Plus, Trash, Globe, 
  HelpCircle, Database, RefreshCw, 
  SlidersHorizontal, AlertCircle, 
  Terminal, ShieldCheck, Heart, CheckCircle2,
  Edit, Copy, Key, ChevronDown, ChevronRight, Folder, FolderPlus
} from 'lucide-react';

// Tauri API Bridge import with mock fallback
type InvokeArgs = Record<string, unknown> | undefined;
type InvokeFn = <T>(cmd: string, args?: InvokeArgs) => Promise<T>;
type MockSendArgs = {
  payload: {
    method: string;
    url: string;
  };
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke: InvokeFn;
    };
  }
}

let invoke: InvokeFn;
try {
  if (window.__TAURI_INTERNALS__) {
    invoke = window.__TAURI_INTERNALS__.invoke;
  } else {
    // Import from core if present
    const tauriCore = await import('@tauri-apps/api/core');
    invoke = tauriCore.invoke;
  }
} catch {
  console.warn("Tauri API not available, running in mock/web mode.");
  invoke = async <T,>(cmd: string, args?: InvokeArgs): Promise<T> => {
    console.log(`[Mock Invoke] ${cmd}`, args);
    
    // Provide realistic mock data for local testing
    if (cmd === 'get_app_settings') {
      return { sidebar_width: 280, response_width: 420 } as T;
    }
    if (cmd === 'get_workspaces') {
      return [
        {
          name: "Default Workspace",
          description: "My primary API testing workspace",
          request_count: 2,
          updated: "2026-06-16",
          requests: [
            {
              id: "req_1",
              name: "Get User Info",
              method: "GET",
              url: "https://httpbin.org/get",
              items: ["Authorization:Bearer secret_token_123", "limit==10"]
            },
            {
              id: "req_2",
              name: "Create User Profile",
              method: "POST",
              url: "https://httpbin.org/post",
              items: ["name=Alice", "role=developer", "hobbies:=['coding', 'design']"]
            }
          ]
        }
      ] as T;
    }
    if (cmd === 'get_secrets') {
      return ["MY_API_KEY", "AWS_SECRET_ACCESS_KEY"] as T;
    }
    if (
      cmd === 'set_secret' || cmd === 'delete_secret' || 
      cmd === 'import_workspace' || cmd === 'export_workspace' ||
      cmd === 'save_test_case' || cmd === 'delete_test_case' ||
      cmd === 'delete_request'
    ) {
      return null as T;
    }
    if (cmd === 'get_environments') {
      return ["development", "production", "staging"] as T;
    }
    if (cmd === 'get_reports') {
      return [
        {
          id: 101,
          module: "Tauri",
          name: "Get User Info",
          summary: "GET https://httpbin.org/get -> 200 OK (184 ms)",
          payload_json: JSON.stringify({ url: "https://httpbin.org/get", status: 200, elapsed_ms: 184 }, null, 2),
          created_at: "2026-06-16T17:15:30Z",
          method: "GET",
          url: "https://httpbin.org/get",
          final_url: "https://httpbin.org/get",
          status: 200,
          reason: "OK",
          elapsed_ms: 184,
          size_bytes: 345,
          content_type: "application/json"
        },
        {
          id: 102,
          module: "CLI",
          name: "System Health Scan",
          summary: "POST https://httpbin.org/post -> 200 OK (312 ms)",
          payload_json: JSON.stringify({ url: "https://httpbin.org/post", status: 200, elapsed_ms: 312 }, null, 2),
          created_at: "2026-06-16T16:40:12Z",
          method: "POST",
          url: "https://httpbin.org/post",
          final_url: "https://httpbin.org/post",
          status: 200,
          reason: "OK",
          elapsed_ms: 312,
          size_bytes: 512,
          content_type: "application/json"
        }
      ] as T;
    }
    if (cmd === 'get_test_cases') {
      return [
        {
          suite: "Auth Service",
          name: "Validate Valid Token",
          source_label: "Tauri Interface",
          method: "GET",
          url: "https://httpbin.org/get",
          items: ["Authorization:Bearer secret_token_123"],
          headers: [],
          expect_status: 200,
          expect_headers: ["Content-Type"],
          expect_json: [],
          expect_body_contains: ["Authorization"],
          max_time_ms: 500,
          created_at: "2026-06-16T12:00:00Z",
          updated_at: "2026-06-16T12:00:00Z"
        },
        {
          suite: "User Profiles",
          name: "Register New User",
          source_label: "Tauri Interface",
          method: "POST",
          url: "https://httpbin.org/post",
          items: ["name=Charlie"],
          headers: [],
          expect_status: 200,
          expect_headers: [],
          expect_json: [],
          expect_body_contains: ["Charlie"],
          max_time_ms: 800,
          created_at: "2026-06-16T12:10:00Z",
          updated_at: "2026-06-16T12:10:00Z"
        }
      ] as T;
    }
    if (cmd === 'run_test_case') {
      await new Promise(r => setTimeout(r, 400));
      return {
        method: "GET",
        url: "https://httpbin.org/get",
        status: 200,
        elapsed_ms: 120,
        passed: true,
        assertions: [
          { assertion: "status == 200", passed: true, details: "status matched 200" },
          { assertion: "elapsed_ms <= 500", passed: true, details: "120 ms matched expected max 500 ms" },
          { assertion: "body contains 'Authorization'", passed: true, details: "substring found" }
        ]
      } as T;
    }
    if (cmd === 'run_test_suite') {
      await new Promise(r => setTimeout(r, 800));
      const suiteName = args?.suite as string || "Auth Service";
      return {
        suite: suiteName,
        generated_at: new Date().toISOString(),
        passed: 2,
        failed: 0,
        cases: [
          { case_name: "Validate Valid Token", passed: true, status: 200, elapsed_ms: 120 },
          { case_name: "Register New User", passed: true, status: 200, elapsed_ms: 210 }
        ]
      } as T;
    }
    if (cmd === 'get_test_runs') {
      return [
        {
          id: 1,
          suite: "Auth Service",
          case_name: "Validate Valid Token",
          passed: true,
          status_code: 200,
          elapsed_ms: 120,
          created_at: new Date(Date.now() - 3600000).toISOString()
        },
        {
          id: 2,
          suite: "User Profiles",
          case_name: "Register New User",
          passed: true,
          status_code: 200,
          elapsed_ms: 210,
          created_at: new Date(Date.now() - 7200000).toISOString()
        }
      ] as T;
    }
    if (cmd === 'send_request') {
      await new Promise(r => setTimeout(r, 600));
      const payload = (args as MockSendArgs | undefined)?.payload ?? {
        method: "GET",
        url: "https://httpbin.org/get"
      };
      return {
        method: payload.method,
        url: payload.url,
        status: 200,
        reason: "OK",
        final_url: payload.url,
        headers: [
          { key: "Content-Type", value: "application/json" },
          { key: "Server", value: "gunicorn/19.9.0" },
          { key: "Access-Control-Allow-Origin", value: "*" }
        ],
        content_type: "application/json",
        body: JSON.stringify({
          args: { limit: "10" },
          headers: {
            Authorization: "Bearer secret_token_123",
            Host: "httpbin.org"
          },
          origin: "127.0.0.1",
          url: payload.url
        }, null, 2),
        body_is_base64: false,
        elapsed_ms: 184,
        elapsed_label: "184 ms",
        size_bytes: 345,
        size_label: "345 B"
      } as T;
    }
    return {} as T;
  };
}

interface RequestDto {
  id: string | null;
  name: string;
  method: string;
  url: string;
  items: string[];
  pre_request_script?: string | null;
  post_response_script?: string | null;
}

interface WorkspaceDto {
  name: string;
  description: string;
  request_count: number;
  updated: string;
  requests: RequestDto[];
}

interface ResponseDto {
  method: string;
  url: string;
  status: number;
  reason: string;
  final_url: string;
  headers: Array<{ key: string; value: string }>;
  content_type: string | null;
  body: string;
  body_is_base64: boolean;
  elapsed_ms: number;
  elapsed_label: string;
  size_bytes: number;
  size_label: string;
  test_results?: string[];
}

interface AppSettings {
  sidebar_width: number;
  response_width: number;
}

interface ReportDto {
  id: number;
  module: string;
  name: string;
  summary: string;
  payload_json: string;
  created_at: string;
  method?: string | null;
  url?: string | null;
  final_url?: string | null;
  status?: number | null;
  reason?: string | null;
  elapsed_ms?: number | null;
  size_bytes?: number | null;
  content_type?: string | null;
}

interface SecurityFindingPayload {
  severity?: string;
  title?: string;
  risk_score?: number | string;
  endpoint?: string;
  impact?: string;
  remediation?: string;
  evidence?: string;
}

interface PerformanceEndpointPayload {
  endpoint?: string;
  samples?: number;
  success_count?: number;
  error_count?: number;
  avg_size_bytes?: number;
  min_ms?: number;
  p50_ms?: number;
  avg_ms?: number;
  p95_ms?: number;
  max_ms?: number;
}

interface ReportPayload {
  source?: string;
  live_scan?: boolean;
  generated_at?: string;
  iterations?: number;
  format?: string;
  request_count?: number;
  output_path?: string;
  method?: string;
  url?: string;
  final_url?: string;
  status?: number;
  reason?: string;
  elapsed_ms?: number;
  size_bytes?: number;
  content_type?: string;
  body?: string;
  findings?: SecurityFindingPayload[];
  endpoints?: PerformanceEndpointPayload[];
}

interface StoredTestCase {
  suite: string;
  name: string;
  source_label: string;
  method: string;
  url: string;
  items: string[];
  headers: Array<[string, string]>;
  expect_status: number | null;
  expect_headers: string[];
  expect_json: string[];
  expect_body_contains: string[];
  max_time_ms: number | null;
  created_at: string;
}

interface TestRunDto {
  id: number;
  suite: string;
  case_name: string;
  passed: boolean;
  status_code: number;
  elapsed_ms: number;
  created_at: string;
}

interface SuiteTreeNode {
  name: string;
  path: string;
  isFolder: boolean;
  children: SuiteTreeNode[];
  cases: StoredTestCase[];
}

interface RequestTreeNode {
  name: string;
  path: string;
  isFolder: boolean;
  children: RequestTreeNode[];
  requests: RequestDto[];
}


interface AssertionResult {
  assertion: string;
  passed: boolean;
  details: string;
}

interface TestReport {
  method: string;
  url: string;
  status: number;
  elapsed_ms: number;
  passed: boolean;
  assertions: AssertionResult[];
}

interface ParamRow {
  key: string;
  value: string;
  enabled: boolean;
}

interface BodyRow {
  key: string;
  value: string;
  type: 'string' | 'json';
  enabled: boolean;
}

function parseRequestItems(items: string[]) {
  const queryParams: ParamRow[] = [];
  const headers: ParamRow[] = [];
  const bodyFields: BodyRow[] = [];
  let authType = 'none';
  let authToken = '';

  for (const item of items) {
    const trimmed = item.trim();
    if (!trimmed) continue;

    if (trimmed.includes('==')) {
      const idx = trimmed.indexOf('==');
      queryParams.push({
        key: trimmed.substring(0, idx),
        value: trimmed.substring(idx + 2),
        enabled: true
      });
      continue;
    }

    if (trimmed.includes(':=')) {
      const idx = trimmed.indexOf(':=');
      bodyFields.push({
        key: trimmed.substring(0, idx),
        value: trimmed.substring(idx + 2),
        type: 'json',
        enabled: true
      });
      continue;
    }

    if (trimmed.includes(':')) {
      const idx = trimmed.indexOf(':');
      const key = trimmed.substring(0, idx).trim();
      const val = trimmed.substring(idx + 1).trim();
      if (key.toLowerCase() === 'authorization' && val.toLowerCase().startsWith('bearer ')) {
        authType = 'bearer';
        authToken = val.substring(7);
      } else {
        headers.push({
          key,
          value: val,
          enabled: true
        });
      }
      continue;
    }

    if (trimmed.includes('=')) {
      const idx = trimmed.indexOf('=');
      bodyFields.push({
        key: trimmed.substring(0, idx),
        value: trimmed.substring(idx + 1),
        type: 'string',
        enabled: true
      });
      continue;
    }
  }

  if (queryParams.length === 0) queryParams.push({ key: '', value: '', enabled: true });
  if (headers.length === 0) headers.push({ key: '', value: '', enabled: true });
  if (bodyFields.length === 0) bodyFields.push({ key: '', value: '', type: 'string', enabled: true });

  return { queryParams, headers, bodyFields, authType, authToken };
}

function buildRequestItems(
  queryParams: ParamRow[],
  headers: ParamRow[],
  bodyFields: BodyRow[],
  authType: string,
  authToken: string
): string[] {
  const items: string[] = [];

  for (const q of queryParams) {
    if (q.enabled && q.key.trim() && q.value.trim()) {
      items.push(`${q.key.trim()}==${q.value}`);
    }
  }

  for (const h of headers) {
    if (h.enabled && h.key.trim() && h.value.trim()) {
      items.push(`${h.key.trim()}:${h.value}`);
    }
  }

  if (authType === 'bearer' && authToken.trim()) {
    items.push(`Authorization:Bearer ${authToken.trim()}`);
  }

  for (const b of bodyFields) {
    if (b.enabled && b.key.trim() && b.value.trim()) {
      if (b.type === 'json') {
        items.push(`${b.key.trim()}:=${b.value}`);
      } else {
        items.push(`${b.key.trim()}=${b.value}`);
      }
    }
  }

  return items;
}

function parseCurlCommand(input: string) {
  const cleanInput = input.trim();
  if (!cleanInput.toLowerCase().startsWith('curl')) {
    return null;
  }
  
  const args: string[] = [];
  let current = '';
  let inDoubleQuote = false;
  let inSingleQuote = false;
  let escaped = false;
  
  for (let i = 0; i < cleanInput.length; i++) {
    const char = cleanInput[i];
    if (escaped) {
      current += char;
      escaped = false;
      continue;
    }
    if (char === '\\') {
      escaped = true;
      continue;
    }
    if (char === '"' && !inSingleQuote) {
      inDoubleQuote = !inDoubleQuote;
      continue;
    }
    if (char === "'" && !inDoubleQuote) {
      inSingleQuote = !inSingleQuote;
      continue;
    }
    if (char === ' ' && !inDoubleQuote && !inSingleQuote) {
      if (current) {
        args.push(current);
        current = '';
      }
    } else {
      current += char;
    }
  }
  if (current) {
    args.push(current);
  }

  let method = 'GET';
  let url = '';
  const headers: { key: string; value: string }[] = [];
  const bodyFields: { key: string; value: string }[] = [];
  let rawBody = '';
  
  for (let i = 1; i < args.length; i++) {
    const arg = args[i];
    if (arg === '-X' || arg === '--request') {
      method = args[++i] || 'GET';
    } else if (arg === '-H' || arg === '--header') {
      const headerStr = args[++i];
      if (headerStr) {
        const parts = headerStr.split(':');
        const key = parts[0]?.trim() || '';
        const value = parts.slice(1).join(':').trim() || '';
        if (key) {
          headers.push({ key, value });
        }
      }
    } else if (arg === '-d' || arg === '--data' || arg === '--data-raw' || arg === '--data-binary') {
      const dataStr = args[++i] || '';
      if (dataStr) {
        rawBody = dataStr;
        const pairs = dataStr.split('&');
        let isFormData = true;
        const tempFields: { key: string; value: string }[] = [];
        for (const pair of pairs) {
          if (pair.includes('=')) {
            const [k, v] = pair.split('=');
            tempFields.push({ key: decodeURIComponent(k || ''), value: decodeURIComponent(v || '') });
          } else {
            isFormData = false;
            break;
          }
        }
        if (isFormData && tempFields.length > 0) {
          bodyFields.push(...tempFields);
        }
      }
      if (method === 'GET') {
        method = 'POST';
      }
    } else if (!arg.startsWith('-')) {
      url = arg.replace(/^["']|["']$/g, '');
    }
  }
  
  return { method: method.toUpperCase(), url, headers, bodyFields, rawBody };
}

function generateCurlCommand(method: string, url: string, headers: ParamRow[], bodyFields: BodyRow[]) {
  let curl = `curl -X ${method} "${url}"`;
  
  // Check if headers already contain content-type: application/json
  const hasJsonHeader = headers.some(h => h.enabled && h.key.trim().toLowerCase() === 'content-type' && h.value.trim().toLowerCase().includes('json'));
  
  for (const h of headers) {
    if (h.enabled && h.key.trim() && h.value.trim()) {
      curl += ` \\\n  -H "${h.key.trim()}: ${h.value.trim()}"`;
    }
  }
  
  const activeBody = bodyFields.filter(f => f.enabled && f.key.trim());
  if (method !== 'GET' && activeBody.length > 0) {
    const hasJsonField = bodyFields.some(f => f.enabled && f.key.trim() && f.type === 'json');
    const isJson = hasJsonHeader || hasJsonField;

    if (isJson) {
      const obj: Record<string, any> = {};
      for (const f of activeBody) {
        const key = f.key.trim();
        if (f.type === 'json') {
          try {
            obj[key] = JSON.parse(f.value);
          } catch {
            obj[key] = f.value;
          }
        } else {
          const val = f.value.trim();
          if (val === 'true') obj[key] = true;
          else if (val === 'false') obj[key] = false;
          else if (val === 'null') obj[key] = null;
          else if (/^\d+$/.test(val)) obj[key] = parseInt(val, 10);
          else if (/^\d+\.\d+$/.test(val)) obj[key] = parseFloat(val);
          else obj[key] = f.value;
        }
      }
      const jsonStr = JSON.stringify(obj);
      if (!hasJsonHeader) {
        curl += ` \\\n  -H "Content-Type: application/json"`;
      }
      curl += ` \\\n  -d '${jsonStr}'`;
    } else {
      const dataStr = activeBody.map(f => `${encodeURIComponent(f.key.trim())}=${encodeURIComponent(f.value)}`).join('&');
      curl += ` \\\n  -d "${dataStr}"`;
    }
  }
  return curl;
}

function isJsonResponse(response: ResponseDto) {
  const contentType = response.content_type?.toLowerCase() || '';
  const body = response.body.trim();
  return contentType.includes('json') || body.startsWith('{') || body.startsWith('[');
}

function isHtmlResponse(response: ResponseDto) {
  const contentType = response.content_type?.toLowerCase() || '';
  return contentType.includes('html') || /^\s*<!doctype html|^\s*<html[\s>]/i.test(response.body);
}

function decodeHtmlEntities(value: string) {
  if (typeof document === 'undefined') return value;
  const textarea = document.createElement('textarea');
  textarea.innerHTML = value;
  return textarea.value;
}

function formatResponseBody(response: ResponseDto) {
  if (response.body_is_base64) {
    return 'Binary response body is base64 encoded. Use the Raw tab to inspect the encoded payload.';
  }

  if (isJsonResponse(response)) {
    try {
      return JSON.stringify(JSON.parse(response.body), null, 2);
    } catch {
      return response.body;
    }
  }

  if (isHtmlResponse(response)) {
    const titleMatch = response.body.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
    const headingMatch = response.body.match(/<h1[^>]*>([\s\S]*?)<\/h1>/i);
    const title = decodeHtmlEntities((headingMatch?.[1] || titleMatch?.[1] || '').replace(/<[^>]+>/g, '').trim());
    const text = decodeHtmlEntities(
      response.body
        .replace(/<script[\s\S]*?<\/script>/gi, '')
        .replace(/<style[\s\S]*?<\/style>/gi, '')
        .replace(/<\/(p|div|h[1-6]|li|tr|br)>/gi, '\n')
        .replace(/<[^>]+>/g, ' ')
        .replace(/[ \t]+/g, ' ')
        .replace(/\n\s+/g, '\n')
        .replace(/\n{3,}/g, '\n\n')
        .trim()
    );

    const summary = [
      `HTTP ${response.status} ${response.reason}`,
      title && title !== `${response.status} ${response.reason}` ? title : '',
      text
    ].filter(Boolean);

    return summary.join('\n\n') || `HTTP ${response.status} ${response.reason}`;
  }

  return response.body;
}

interface ReportMeta {
  method?: string;
  url?: string;
  final_url?: string;
  status?: number;
  reason?: string;
  elapsed_ms?: number;
  size_bytes?: number;
  content_type?: string;
}

function parseReportPayload(report: Pick<ReportDto, 'payload_json'>): ReportPayload | null {
  try {
    return JSON.parse(report.payload_json) as ReportPayload;
  } catch {
    return null;
  }
}

function reportMeta(report: ReportDto): ReportMeta {
  const payload = parseReportPayload(report);
  return {
    method: report.method || payload?.method,
    url: report.url || payload?.url,
    final_url: report.final_url || payload?.final_url,
    status: report.status ?? payload?.status,
    reason: report.reason || payload?.reason,
    elapsed_ms: report.elapsed_ms ?? payload?.elapsed_ms,
    size_bytes: report.size_bytes ?? payload?.size_bytes,
    content_type: report.content_type || payload?.content_type,
  };
}

function historyKey(method?: string, url?: string) {
  if (!method || !url) return '';
  return `${method.toUpperCase()} ${url.trim()}`;
}

function formatMs(value?: number) {
  if (typeof value !== 'number') return 'n/a';
  return value >= 1000 ? `${(value / 1000).toFixed(value >= 10000 ? 1 : 2)} s` : `${value} ms`;
}

function formatBytes(value?: number) {
  if (typeof value !== 'number') return 'n/a';
  if (value < 1024) return `${value} B`;
  const kb = value / 1024;
  if (kb < 1024) return `${kb.toFixed(kb >= 100 ? 0 : 1)} KB`;
  return `${(kb / 1024).toFixed(1)} MB`;
}

function exportExtension(format: string) {
  switch (format) {
    case 'zapreq':
      return 'zapreq.json';
    case 'openapi':
      return 'openapi.json';
    case 'postman':
    default:
      return 'postman.json';
  }
}

function slugFilename(value: string) {
  const slug = value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return slug || 'workspace';
}

function defaultExportFilename(workspaceName: string, format: string) {
  return `${slugFilename(workspaceName)}.${exportExtension(format)}`;
}

function isJsonText(source?: string | null) {
  if (!source) return false;
  const trimmed = source.trim();
  if (!trimmed.startsWith('{') && !trimmed.startsWith('[')) return false;
  try {
    JSON.parse(trimmed);
    return true;
  } catch {
    return false;
  }
}

function formattedJson(source: string) {
  try {
    return JSON.stringify(JSON.parse(source), null, 2);
  } catch {
    return source;
  }
}

function highlightJson(source: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  const tokenPattern = /("(?:\\u[\da-fA-F]{4}|\\[^u]|[^\\"])*"|[-]?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|true|false|null|[{}[\],:])/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = tokenPattern.exec(source)) !== null) {
    const token = match[0];
    if (match.index > lastIndex) {
      nodes.push(source.slice(lastIndex, match.index));
    }

    const nextChar = source.slice(match.index + token.length).trimStart()[0];
    let className = 'json-punctuation';
    if (token.startsWith('"')) {
      className = nextChar === ':' ? 'json-key' : 'json-string';
    } else if (/^-?\d/.test(token)) {
      className = 'json-number';
    } else if (token === 'true' || token === 'false') {
      className = 'json-boolean';
    } else if (token === 'null') {
      className = 'json-null';
    }

    nodes.push(
      <span className={className} key={`${match.index}-${token}`}>
        {token}
      </span>
    );
    lastIndex = match.index + token.length;
  }

  if (lastIndex < source.length) {
    nodes.push(source.slice(lastIndex));
  }

  return nodes;
}

function JsonCode({ source }: { source: string }) {
  return (
    <pre className="json-viewer">
      <code>{highlightJson(formattedJson(source))}</code>
    </pre>
  );
}

function buildRequestTree(requests: RequestDto[], emptyFolders: string[] = []): RequestTreeNode[] {
  const root: RequestTreeNode = { name: '', path: '', isFolder: true, children: [], requests: [] };

  const ensurePath = (folderPath: string) => {
    const segments = folderPath.split('/').map(s => s.trim()).filter(Boolean);
    let current = root;
    let currentPath = '';
    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      currentPath = currentPath ? `${currentPath} / ${seg}` : seg;
      let child = current.children.find(c => c.name === seg);
      if (!child) {
        child = {
          name: seg,
          path: currentPath,
          isFolder: true,
          children: [],
          requests: []
        };
        current.children.push(child);
      }
      current = child;
    }
  };

  for (const ef of emptyFolders) {
    ensurePath(ef);
  }

  for (const req of requests) {
    const name = req.name || 'Untitled Request';
    const segments = name.split('/').map(s => s.trim()).filter(Boolean);
    
    let current = root;
    let currentPath = '';
    
    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      currentPath = currentPath ? `${currentPath} / ${seg}` : seg;
      
      const isLast = i === segments.length - 1;
      let child = current.children.find(c => c.name === seg);
      if (!child) {
        child = {
          name: seg,
          path: currentPath,
          isFolder: !isLast,
          children: [],
          requests: []
        };
        current.children.push(child);
      } else {
        if (!isLast) {
          child.isFolder = true;
        }
      }
      
      if (isLast) {
        child.requests.push(req);
      }
      current = child;
    }
  }

  const sortTree = (nodes: RequestTreeNode[]) => {
    nodes.sort((a, b) => {
      if (a.isFolder && !b.isFolder) return -1;
      if (!a.isFolder && b.isFolder) return 1;
      return a.name.localeCompare(b.name);
    });
    for (const n of nodes) {
      if (n.children.length > 0) {
        sortTree(n.children);
      }
    }
  };

  sortTree(root.children);
  return root.children;
}

function buildSuiteTree(testCases: StoredTestCase[]): SuiteTreeNode[] {
  const root: SuiteTreeNode = { name: '', path: '', isFolder: true, children: [], cases: [] };

  for (const tc of testCases) {
    const suiteName = tc.suite || 'Default';
    const segments = suiteName.split('/').map(s => s.trim()).filter(Boolean);
    
    let current = root;
    let currentPath = '';
    
    for (let i = 0; i < segments.length; i++) {
      const seg = segments[i];
      currentPath = currentPath ? `${currentPath} / ${seg}` : seg;
      
      let child = current.children.find(c => c.name === seg);
      if (!child) {
        child = {
          name: seg,
          path: currentPath,
          isFolder: true,
          children: [],
          cases: []
        };
        current.children.push(child);
      }
      
      if (i === segments.length - 1) {
        child.cases.push(tc);
      }
      current = child;
    }
  }

  const sortTree = (nodes: SuiteTreeNode[]) => {
    nodes.sort((a, b) => a.name.localeCompare(b.name));
    for (const n of nodes) {
      if (n.children.length > 0) {
        sortTree(n.children);
      }
    }
  };

  sortTree(root.children);
  return root.children;
}

function TestCasesDonutChart({ passed, failed, untested }: { passed: number, failed: number, untested: number }) {
  const total = passed + failed + untested;
  if (total === 0) {
    return (
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', height: '180px', color: 'var(--text-muted)' }}>
        <span style={{ fontSize: '13px' }}>No test cases saved to database.</span>
      </div>
    );
  }
  
  const r = 40;
  const circumference = 2 * Math.PI * r;
  
  const pPct = passed / total;
  const fPct = failed / total;
  const uPct = untested / total;
  
  const pLen = pPct * circumference;
  const fLen = fPct * circumference;
  
  const pOffset = circumference;
  const fOffset = circumference - pLen;
  const uOffset = circumference - pLen - fLen;
  
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: '48px', padding: '16px', backgroundColor: 'var(--bg-secondary)', borderRadius: '8px', border: '1px solid var(--border-color)', minHeight: '180px' }}>
      <div style={{ position: 'relative', width: '130px', height: '130px', flexShrink: 0 }}>
        <svg width="100%" height="100%" viewBox="0 0 100 100" style={{ transform: 'rotate(-90deg)' }}>
          <circle
            cx="50"
            cy="50"
            r={r}
            fill="transparent"
            stroke="rgba(255,255,255,0.05)"
            strokeWidth="10"
          />
          {untested > 0 && (
            <circle
              cx="50"
              cy="50"
              r={r}
              fill="transparent"
              stroke="#6b7280"
              strokeWidth="10"
              strokeDasharray={circumference}
              strokeDashoffset={uOffset}
              strokeLinecap="round"
              style={{ transition: 'stroke-dashoffset 0.3s ease' }}
            />
          )}
          {failed > 0 && (
            <circle
              cx="50"
              cy="50"
              r={r}
              fill="transparent"
              stroke="var(--color-delete)"
              strokeWidth="10"
              strokeDasharray={circumference}
              strokeDashoffset={fOffset}
              strokeLinecap="round"
              style={{ transition: 'stroke-dashoffset 0.3s ease' }}
            />
          )}
          {passed > 0 && (
            <circle
              cx="50"
              cy="50"
              r={r}
              fill="transparent"
              stroke="var(--color-get)"
              strokeWidth="10"
              strokeDasharray={circumference}
              strokeDashoffset={pOffset}
              strokeLinecap="round"
              style={{ transition: 'stroke-dashoffset 0.3s ease' }}
            />
          )}
        </svg>
        <div style={{
          position: 'absolute',
          top: 0, left: 0, right: 0, bottom: 0,
          display: 'flex', flexDirection: 'column',
          alignItems: 'center', justifyContent: 'center'
        }}>
          <span style={{ fontSize: '20px', fontWeight: 'bold', color: 'var(--text-primary)' }}>{total}</span>
          <span style={{ fontSize: '9px', color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.05em' }}>Tests</span>
        </div>
      </div>
      
      <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', flex: 1 }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '6px 12px', backgroundColor: 'rgba(16, 185, 129, 0.08)', borderLeft: '3px solid var(--color-get)', borderRadius: '0 4px 4px 0' }}>
          <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Passed</span>
          <span style={{ fontSize: '15px', fontWeight: 'bold', color: 'var(--color-get)' }}>{passed} ({Math.round(pPct*100)}%)</span>
        </div>
        
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '6px 12px', backgroundColor: 'rgba(239, 68, 68, 0.08)', borderLeft: '3px solid var(--color-delete)', borderRadius: '0 4px 4px 0' }}>
          <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Failed</span>
          <span style={{ fontSize: '15px', fontWeight: 'bold', color: 'var(--color-delete)' }}>{failed} ({Math.round(fPct*100)}%)</span>
        </div>
        
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '6px 12px', backgroundColor: 'rgba(107, 114, 128, 0.08)', borderLeft: '3px solid #6b7280', borderRadius: '0 4px 4px 0' }}>
          <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Untested</span>
          <span style={{ fontSize: '15px', fontWeight: 'bold', color: '#9ca3af' }}>{untested} ({Math.round(uPct*100)}%)</span>
        </div>
      </div>
    </div>
  );
}

export default function App() {
  // Navigation Shell State
  const [activeView, setActiveView] = useState<'collections' | 'tests' | 'reports'>('collections');

  // App Shell Layout dimensions
  const [sidebarWidth, setSidebarWidth] = useState(280);
  const [responseWidth, setResponseWidth] = useState(420);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const [isResizingResponse, setIsResizingResponse] = useState(false);

  // Core Data States
  const [workspaces, setWorkspaces] = useState<WorkspaceDto[]>([]);
  const [activeWorkspaceName, setActiveWorkspaceName] = useState("");
  const [environments, setEnvironments] = useState<string[]>([]);
  const [selectedEnv, setSelectedEnv] = useState("none");
  const [reports, setReports] = useState<ReportDto[]>([]);
  const [selectedReport, setSelectedReport] = useState<ReportDto | null>(null);
  const [testCases, setTestCases] = useState<StoredTestCase[]>([]);
  const [selectedTestCase, setSelectedTestCase] = useState<StoredTestCase | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [runReport, setRunReport] = useState<TestReport | null>(null);
  const [isRunningTest, setIsRunningTest] = useState(false);
  const [customEmptyFolders, setCustomEmptyFolders] = useState<string[]>([]);
  const [dragOverFolder, setDragOverFolder] = useState<string | null>(null);
  const [dragOverRoot, setDragOverRoot] = useState(false);

  // Active collection/request details state
  const [activeRequest, setActiveRequest] = useState<RequestDto | null>(null);
  const [requestName, setRequestName] = useState("New Request");
  const [requestMethod, setRequestMethod] = useState("GET");
  const [requestUrl, setRequestUrl] = useState("");
  const [queryParams, setQueryParams] = useState<ParamRow[]>([{ key: '', value: '', enabled: true }]);
  const [headers, setHeaders] = useState<ParamRow[]>([{ key: '', value: '', enabled: true }]);
  const [bodyFields, setBodyFields] = useState<BodyRow[]>([{ key: '', value: '', type: 'string', enabled: true }]);
  const [authType, setAuthType] = useState("none");
  const [authToken, setAuthToken] = useState("");
  const [requestTab, setRequestTab] = useState("params");

  // Scripting State
  const [preRequestScript, setPreRequestScript] = useState("");
  const [postResponseScript, setPostResponseScript] = useState("");

  // Secrets State
  const [secrets, setSecrets] = useState<string[]>([]);
  const [newSecretKey, setNewSecretKey] = useState("");
  const [newSecretVal, setNewSecretVal] = useState("");

  // Import / Export State
  const [showImportDialog, setShowImportDialog] = useState(false);
  const [showExportDialog, setShowExportDialog] = useState(false);
  const [importWsName, setImportWsName] = useState("");
  const [importFilePath, setImportFilePath] = useState("");
  const [exportFilePath, setExportFilePath] = useState("");
  const [exportFormat, setExportFormat] = useState("postman");

  // Workspace Creation Dialog
  const [showCreateWorkspace, setShowCreateWorkspace] = useState(false);
  const [newWsName, setNewWsName] = useState("");
  const [newWsDesc, setNewWsDesc] = useState("");

  // Test Case Dialog
  const [showAddTestDialog, setShowAddTestDialog] = useState(false);
  const [testSuiteName, setTestSuiteName] = useState("");
  const [testCaseName, setTestCaseName] = useState("");
  const [expectStatus, setExpectStatus] = useState("200");
  const [expectMaxTime, setExpectMaxTime] = useState("500");
  const [expectContains, setExpectContains] = useState("");
  
  // Curl Clipboard Feedback State
  const [copiedCurl, setCopiedCurl] = useState(false);

  // Custom request info for test cases when not created from builder
  const [selectedRequestForTestCaseId, setSelectedRequestForTestCaseId] = useState("custom");
  const [tcRequestMethod, setTcRequestMethod] = useState("GET");
  const [tcRequestUrl, setTcRequestUrl] = useState("");
  const [tcRequestItems, setTcRequestItems] = useState<string[]>([]);

  // Suite Selection, History, and Runner State
  const [selectedSuitePath, setSelectedSuitePath] = useState<string | null>(null);
  const [expandedFolders, setExpandedFolders] = useState<Record<string, boolean>>({});
  const [expandedRequestFolders, setExpandedRequestFolders] = useState<Record<string, boolean>>({});
  const [testRuns, setTestRuns] = useState<TestRunDto[]>([]);
  const [suiteProgress, setSuiteProgress] = useState<{ current: number; total: number; caseName: string } | null>(null);
  const [isRunningSuite, setIsRunningSuite] = useState(false);
  const [suiteReport, setSuiteReport] = useState<{
    suite: string;
    generated_at: string;
    passed: number;
    failed: number;
    cases: Array<{ case_name: string; passed: boolean; status: number; elapsed_ms: number }>;
  } | null>(null);

  // Workspace Renaming Dialog
  const [showRenameWorkspace, setShowRenameWorkspace] = useState(false);
  const [newWsRenameName, setNewWsRenameName] = useState("");

  // Response Pane State (Collections View)
  const [response, setResponse] = useState<ResponseDto | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [errorText, setErrorText] = useState("");
  const [responseTab, setResponseTab] = useState("body");

  // General Settings Dialog
  const [showSettings, setShowSettings] = useState(false);
  const [settingsTab, setSettingsTab] = useState<'general' | 'secrets'>('general');

  // Ref selectors for drag handling
  const containerRef = useRef<HTMLDivElement>(null);

  // Load Workspaces, environments, reports, and tests
  const loadInitialData = useCallback(async () => {
    try {
      const ws = await invoke<WorkspaceDto[]>('get_workspaces');
      setWorkspaces(ws);
      if (ws.length > 0 && !activeWorkspaceName) {
        setActiveWorkspaceName(ws[0].name);
      }

      const envs = await invoke<string[]>('get_environments');
      setEnvironments(envs);

      const settings = await invoke<AppSettings>('get_app_settings');
      if (settings) {
        if (settings.sidebar_width) setSidebarWidth(Math.max(240, Math.min(360, settings.sidebar_width)));
        if (settings.response_width) setResponseWidth(Math.max(340, Math.min(620, settings.response_width)));
      }

      const dbReports = await invoke<ReportDto[]>('get_reports');
      setReports(dbReports);

      const dbTests = await invoke<StoredTestCase[]>('get_test_cases');
      setTestCases(dbTests);

      const dbRuns = await invoke<TestRunDto[]>('get_test_runs');
      setTestRuns(dbRuns);

      try {
        const secretKeys = await invoke<string[]>('get_secrets');
        setSecrets(secretKeys);
      } catch (e) {
        console.warn("Failed to load secrets", e);
      }
    } catch (e) {
      console.error("Failed to load initial data", e);
    }
  }, [activeWorkspaceName]);

  useEffect(() => {
    void Promise.resolve().then(() => loadInitialData());
  }, [loadInitialData]);

  // Load request details when active request changes
  const loadRequestDetails = useCallback((req: RequestDto) => {
    setActiveRequest(req);
    setRequestName(req.name);
    setRequestMethod(req.method);
    setRequestUrl(req.url);

    const parsed = parseRequestItems(req.items);
    setQueryParams(parsed.queryParams);
    setHeaders(parsed.headers);
    setBodyFields(parsed.bodyFields);
    setAuthType(parsed.authType);
    setAuthToken(parsed.authToken);
    setPreRequestScript(req.pre_request_script || '');
    setPostResponseScript(req.post_response_script || '');
    setResponse(null);
    setErrorText("");
  }, []);

  // Set default first request on load
  useEffect(() => {
    if (!activeWorkspaceName) return;
    const currentWs = workspaces.find(w => w.name === activeWorkspaceName);
    if (currentWs && currentWs.requests.length > 0 && !activeRequest) {
      void Promise.resolve().then(() => loadRequestDetails(currentWs.requests[0]));
    }
  }, [workspaces, activeWorkspaceName, activeRequest, loadRequestDetails]);

  // Save current app settings to backend
  const persistSettings = useCallback(async (sidebar: number, responsePane: number) => {
    try {
      await invoke('save_app_settings', {
        settings: {
          sidebar_width: sidebar,
          response_width: responsePane
        }
      });
    } catch (e) {
      console.error("Failed to save settings", e);
    }
  }, []);

  // Drag handlers for resizing panels
  const handleMouseDownSidebar = () => {
    setIsResizingSidebar(true);
  };

  const handleMouseDownResponse = () => {
    setIsResizingResponse(true);
  };

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (isResizingSidebar && containerRef.current) {
      const rect = containerRef.current.getBoundingClientRect();
      const newWidth = Math.max(240, Math.min(360, e.clientX - rect.left));
      setSidebarWidth(newWidth);
    }
    if (isResizingResponse && containerRef.current) {
      const rect = containerRef.current.getBoundingClientRect();
      const newWidth = Math.max(340, Math.min(620, rect.right - e.clientX));
      setResponseWidth(newWidth);
    }
  }, [isResizingSidebar, isResizingResponse]);

  const handleMouseUp = useCallback(() => {
    if (isResizingSidebar || isResizingResponse) {
      setIsResizingSidebar(false);
      setIsResizingResponse(false);
      persistSettings(sidebarWidth, responseWidth);
    }
  }, [isResizingSidebar, isResizingResponse, sidebarWidth, responseWidth, persistSettings]);

  useEffect(() => {
    if (isResizingSidebar || isResizingResponse) {
      window.addEventListener('mousemove', handleMouseMove);
      window.addEventListener('mouseup', handleMouseUp);
    }
    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isResizingSidebar, isResizingResponse, handleMouseMove, handleMouseUp]);

  // Add workspace action
  const handleCreateWorkspace = async () => {
    if (!newWsName.trim()) return;
    try {
      const newWs = await invoke<WorkspaceDto>('create_workspace', {
        payload: {
          name: newWsName,
          description: newWsDesc || null
        }
      });
      setWorkspaces(prev => [...prev, newWs]);
      setActiveWorkspaceName(newWs.name);
      setShowCreateWorkspace(false);
      setNewWsName("");
      setNewWsDesc("");
    } catch (e) {
      alert(`Error creating workspace: ${e}`);
    }
  };

  // Rename workspace action
  const handleRenameWorkspace = async () => {
    if (!newWsRenameName.trim() || newWsRenameName === activeWorkspaceName) return;
    try {
      await invoke('rename_workspace', {
        oldName: activeWorkspaceName,
        newName: newWsRenameName.trim()
      });
      const ws = await invoke<WorkspaceDto[]>('get_workspaces');
      setWorkspaces(ws);
      setActiveWorkspaceName(newWsRenameName.trim());
      setShowRenameWorkspace(false);
    } catch (e) {
      alert(`Error renaming workspace: ${e}`);
    }
  };

  // Save Test Case
  const handleSaveTestCase = async () => {
    if (!testSuiteName.trim() || !testCaseName.trim()) {
      alert("Suite name and case name are required.");
      return;
    }
    
    let method = tcRequestMethod;
    let url = tcRequestUrl;
    let items = tcRequestItems;

    if (selectedRequestForTestCaseId && selectedRequestForTestCaseId !== "custom") {
      const req = currentWorkspace?.requests.find(r => r.id === selectedRequestForTestCaseId);
      if (req) {
        method = req.method;
        url = req.url;
        items = req.items;
      }
    }

    const payload = {
      suite: testSuiteName.trim(),
      name: testCaseName.trim(),
      method: method,
      url: url,
      items: items,
      expect_status: expectStatus.trim() ? parseInt(expectStatus.trim(), 10) : null,
      expect_headers: [],
      expect_json: [],
      expect_body_contains: expectContains.trim() ? [expectContains.trim()] : [],
      max_time_ms: expectMaxTime.trim() ? parseInt(expectMaxTime.trim(), 10) : null,
    };

    try {
      await invoke('save_test_case', { payload });
      alert(`Test case '${testCaseName}' saved in suite '${testSuiteName}'!`);
      setShowAddTestDialog(false);
      
      const dbTests = await invoke<StoredTestCase[]>('get_test_cases');
      setTestCases(dbTests);
    } catch (e) {
      alert(`Failed to save test case: ${e}`);
    }
  };

  // Delete Test Case
  const handleDeleteTestCase = async (suite: string, name: string) => {
    if (!confirm(`Are you sure you want to delete test case '${name}' from suite '${suite}'?`)) {
      return;
    }
    try {
      await invoke('delete_test_case', { suite, name });
      const dbTests = await invoke<StoredTestCase[]>('get_test_cases');
      setTestCases(dbTests);
      
      if (selectedTestCase && selectedTestCase.suite === suite && selectedTestCase.name === name) {
        setSelectedTestCase(null);
        setRunReport(null);
      }
    } catch (e) {
      alert(`Failed to delete test case: ${e}`);
    }
  };

  // Run Stored Test Suite (Recursive sequentially)
  const handleRunTestSuite = async (suitePath: string) => {
    const casesToRun = testCases.filter(tc => tc.suite === suitePath || tc.suite.startsWith(suitePath + ' / '));
    if (casesToRun.length === 0) return;
    
    setIsRunningSuite(true);
    setSuiteProgress({ current: 0, total: casesToRun.length, caseName: "" });
    setSuiteReport(null);
    
    const results: Array<{ case_name: string; passed: boolean; status: number; elapsed_ms: number }> = [];
    let passed = 0;
    let failed = 0;
    
    try {
      for (let i = 0; i < casesToRun.length; i++) {
        const tc = casesToRun[i];
        setSuiteProgress({ current: i + 1, total: casesToRun.length, caseName: tc.name });
        
        try {
          const report = await invoke<TestReport>('run_test_case', {
            payload: { suite: tc.suite, name: tc.name }
          });
          results.push({
            case_name: tc.name,
            passed: report.passed,
            status: report.status,
            elapsed_ms: report.elapsed_ms
          });
          if (report.passed) passed++; else failed++;
        } catch (e) {
          results.push({
            case_name: tc.name,
            passed: false,
            status: 0,
            elapsed_ms: 0
          });
          failed++;
        }
      }
      
      setSuiteReport({
        suite: suitePath,
        generated_at: new Date().toISOString(),
        passed,
        failed,
        cases: results
      });
      
      const dbRuns = await invoke<TestRunDto[]>('get_test_runs');
      setTestRuns(dbRuns);
    } catch (e) {
      alert(`Failed to execute suite: ${e}`);
    } finally {
      setIsRunningSuite(false);
      setSuiteProgress(null);
    }
  };

  // Delete Test Suite Folder (Recursive)
  const handleDeleteTestSuite = async (suitePath: string) => {
    const casesToDelete = testCases.filter(tc => tc.suite === suitePath || tc.suite.startsWith(suitePath + ' / '));
    if (casesToDelete.length === 0) return;
    if (!confirm(`Are you sure you want to delete folder '${suitePath}' and all its ${casesToDelete.length} test cases?`)) {
      return;
    }
    try {
      for (const tc of casesToDelete) {
        await invoke('delete_test_case', { suite: tc.suite, name: tc.name });
      }
      const dbTests = await invoke<StoredTestCase[]>('get_test_cases');
      setTestCases(dbTests);
      const dbRuns = await invoke<TestRunDto[]>('get_test_runs');
      setTestRuns(dbRuns);
      
      if (selectedSuitePath === suitePath || (selectedSuitePath && selectedSuitePath.startsWith(suitePath + ' / '))) {
        setSelectedSuitePath(null);
        setSuiteReport(null);
      }
    } catch (e) {
      alert(`Failed to delete suite: ${e}`);
    }
  };

  // Import Collection / Workspace
  const handleImportWorkspace = async () => {
    if (!importWsName.trim() || !importFilePath.trim()) {
      alert("Please enter both a workspace name and a file path.");
      return;
    }
    try {
      await invoke('import_workspace', { 
        name: importWsName.trim(), 
        path: importFilePath.trim() 
      });
      const ws = await invoke<WorkspaceDto[]>('get_workspaces');
      setWorkspaces(ws);
      setActiveWorkspaceName(importWsName.trim());
      setActiveRequest(null);
      alert("Workspace imported successfully.");
      setShowImportDialog(false);
      setImportWsName("");
      setImportFilePath("");
    } catch (e) {
      alert(`Import failed: ${String(e).replace(/^Error:\s*/, '')}`);
    }
  };

  // Export Collection / Workspace
  const handleExportWorkspace = async () => {
    if (!exportFilePath.trim()) {
      alert("Please enter a destination file path.");
      return;
    }
    try {
      const exportedPath = await invoke<string>('export_workspace', {
        name: activeWorkspaceName,
        path: exportFilePath.trim(),
        format: exportFormat
      });
      alert(`Workspace exported successfully to ${exportedPath}`);
      setShowExportDialog(false);
      setExportFilePath("");
    } catch (e) {
      alert(`Export failed: ${String(e).replace(/^Error:\s*/, '')}`);
    }
  };

  // Add a new Secret key-value
  const handleAddSecret = async () => {
    if (!newSecretKey.trim() || !newSecretVal.trim()) {
      alert("Please fill in both key and value.");
      return;
    }
    try {
      await invoke('set_secret', { 
        key: newSecretKey.trim(), 
        value: newSecretVal.trim() 
      });
      setNewSecretKey("");
      setNewSecretVal("");
      const secretKeys = await invoke<string[]>('get_secrets');
      setSecrets(secretKeys);
      alert("Secret saved successfully!");
    } catch (e) {
      alert(`Failed to save secret: ${e}`);
    }
  };

  // Delete a Secret key
  const handleDeleteSecret = async (key: string) => {
    if (!confirm(`Are you sure you want to delete secret '${key}'?`)) return;
    try {
      await invoke('delete_secret', { key });
      const secretKeys = await invoke<string[]>('get_secrets');
      setSecrets(secretKeys);
    } catch (e) {
      alert(`Failed to delete secret: ${e}`);
    }
  };

  // Add new request action
  const handleCreateRequest = async () => {
    const defaultName = `Request ${Date.now().toString().slice(-4)}`;
    try {
      const saved = await invoke<RequestDto>('save_request', {
        payload: {
          id: null,
          workspace: activeWorkspaceName,
          name: defaultName,
          method: "GET",
          url: "https://httpbin.org/get",
          items: []
        }
      });
      
      setWorkspaces(prev => prev.map(w => {
        if (w.name === activeWorkspaceName) {
          return { ...w, requests: [...w.requests, saved] };
        }
        return w;
      }));
      loadRequestDetails(saved);
    } catch (e) {
      alert(`Error creating request: ${e}`);
    }
  };

  const handleCreateRequestFolder = () => {
    const name = prompt("Enter folder path (e.g. 'Auth' or 'Billing / Nested'):");
    if (!name || !name.trim()) return;
    const cleanName = name.split('/').map(s => s.trim()).filter(Boolean).join(' / ');
    if (!cleanName) return;
    
    setCustomEmptyFolders(prev => {
      if (prev.includes(cleanName)) return prev;
      return [...prev, cleanName];
    });
  };

  const handleCreateRequestInFolder = async (folderPath: string) => {
    const defaultName = `${folderPath} / Request ${Date.now().toString().slice(-4)}`;
    try {
      const saved = await invoke<RequestDto>('save_request', {
        payload: {
          id: null,
          workspace: activeWorkspaceName,
          name: defaultName,
          method: "GET",
          url: "https://httpbin.org/get",
          items: []
        }
      });
      
      setWorkspaces(prev => prev.map(w => {
        if (w.name === activeWorkspaceName) {
          return { ...w, requests: [...w.requests, saved] };
        }
        return w;
      }));
      loadRequestDetails(saved);
    } catch (e) {
      alert(`Error creating request inside folder: ${e}`);
    }
  };

  const handleMoveRequestToFolder = async (requestId: string, targetFolder: string) => {
    if (!currentWorkspace) return;
    const req = currentWorkspace.requests.find(r => r.id === requestId);
    if (!req) return;
    
    const parts = req.name.split(' / ').map(s => s.trim()).filter(Boolean);
    const leafName = parts[parts.length - 1] || 'Untitled';
    const newName = targetFolder ? `${targetFolder} / ${leafName}` : leafName;
    
    if (req.name === newName) return;
    
    const oldFolder = parts.slice(0, parts.length - 1).join(' / ');
    
    try {
      const saved = await invoke<RequestDto>('save_request', {
        payload: {
          id: req.id,
          workspace: activeWorkspaceName,
          name: newName,
          method: req.method,
          url: req.url,
          items: req.items,
          pre_request_script: req.pre_request_script || null,
          post_response_script: req.post_response_script || null
        }
      });
      
      if (targetFolder) {
        setCustomEmptyFolders(prev => prev.filter(f => f !== targetFolder));
      }
      
      if (oldFolder && currentWorkspace) {
        const remainingInOldFolder = currentWorkspace.requests.filter(r => 
          r.id !== req.id && (r.name === oldFolder || r.name.startsWith(oldFolder + ' / '))
        );
        if (remainingInOldFolder.length === 0) {
          setCustomEmptyFolders(prev => {
            if (prev.includes(oldFolder)) return prev;
            return [...prev, oldFolder];
          });
        }
      }
      
      if (activeRequest && activeRequest.id === req.id) {
        setActiveRequest(saved);
        setRequestName(newName);
      }
      
      loadInitialData();
    } catch (e) {
      alert(`Failed to move request: ${e}`);
    }
  };

  const handleDeleteRequest = async (reqId: string, name: string) => {
    if (!confirm(`Are you sure you want to delete request '${name}'?`)) {
      return;
    }
    const parts = name.split(' / ').map(s => s.trim()).filter(Boolean);
    const parentFolder = parts.slice(0, parts.length - 1).join(' / ');
    
    try {
      await invoke('delete_request', { workspace: activeWorkspaceName, id: reqId });
      
      if (parentFolder && currentWorkspace) {
        const remainingInFolder = currentWorkspace.requests.filter(r => 
          r.id !== reqId && (r.name === parentFolder || r.name.startsWith(parentFolder + ' / '))
        );
        if (remainingInFolder.length === 0) {
          setCustomEmptyFolders(prev => {
            if (prev.includes(parentFolder)) return prev;
            return [...prev, parentFolder];
          });
        }
      }
      
      if (activeRequest && activeRequest.id === reqId) {
        setActiveRequest(null);
        setResponse(null);
      }
      loadInitialData();
    } catch (e) {
      alert(`Failed to delete request: ${e}`);
    }
  };

  const handleDeleteRequestFolder = async (folderPath: string) => {
    if (!currentWorkspace) return;
    const requestsToDelete = currentWorkspace.requests.filter(r => 
      r.name === folderPath || r.name.startsWith(folderPath + ' / ')
    );
    
    const isEmpty = requestsToDelete.length === 0;
    const msg = isEmpty 
      ? `Are you sure you want to delete empty folder '${folderPath}'?`
      : `Are you sure you want to delete folder '${folderPath}' and all its ${requestsToDelete.length} requests?`;
      
    if (!confirm(msg)) {
      return;
    }
    
    try {
      if (!isEmpty) {
        for (const r of requestsToDelete) {
          if (r.id) {
            await invoke('delete_request', { workspace: activeWorkspaceName, id: r.id });
          }
        }
      }
      
      setCustomEmptyFolders(prev => prev.filter(f => f !== folderPath && !f.startsWith(folderPath + ' / ')));
      
      const activeDeleted = requestsToDelete.some(r => activeRequest && r.id === activeRequest.id);
      if (activeDeleted) {
        setActiveRequest(null);
        setResponse(null);
      }
      loadInitialData();
    } catch (e) {
      alert(`Failed to delete request folder: ${e}`);
    }
  };

  const handleSaveRequest = async () => {
    if (!activeRequest) return;
    const items = buildRequestItems(queryParams, headers, bodyFields, authType, authToken);
    
    const oldParts = activeRequest.name.split(' / ').map(s => s.trim()).filter(Boolean);
    const oldFolder = oldParts.slice(0, oldParts.length - 1).join(' / ');
    
    try {
      const saved = await invoke<RequestDto>('save_request', {
        payload: {
          id: activeRequest.id,
          workspace: activeWorkspaceName,
          name: requestName,
          method: requestMethod,
          url: requestUrl,
          items,
          pre_request_script: preRequestScript || null,
          post_response_script: postResponseScript || null
        }
      });
      
      const newParts = requestName.split(' / ').map(s => s.trim()).filter(Boolean);
      const newFolder = newParts.slice(0, newParts.length - 1).join(' / ');
      if (newFolder) {
        setCustomEmptyFolders(prev => prev.filter(f => f !== newFolder));
      }
      
      if (oldFolder && oldFolder !== newFolder && currentWorkspace) {
        const remainingInOldFolder = currentWorkspace.requests.filter(r => 
          r.id !== activeRequest.id && (r.name === oldFolder || r.name.startsWith(oldFolder + ' / '))
        );
        if (remainingInOldFolder.length === 0) {
          setCustomEmptyFolders(prev => {
            if (prev.includes(oldFolder)) return prev;
            return [...prev, oldFolder];
          });
        }
      }
      
      setWorkspaces(prev => prev.map(w => {
        if (w.name === activeWorkspaceName) {
          return {
            ...w,
            requests: w.requests.map(r => {
              if (activeRequest.id && r.id === activeRequest.id) return saved;
              if (!activeRequest.id && r.name === activeRequest.name) return saved;
              return r;
            })
          };
        }
        return w;
      }));
      setActiveRequest(saved);
    } catch (e) {
      alert(`Error saving request: ${e}`);
    }
  };

  // Execute request via Tauri/Rust HTTP engine
  const handleSendRequest = async () => {
    const trimmedUrl = requestUrl.trim();
    if (!trimmedUrl) {
      setErrorText("Enter a request URL before sending.");
      setResponse(null);
      return;
    }

    setIsLoading(true);
    setErrorText("");
    setResponse(null);
    setResponseTab("body");

    const items = buildRequestItems(queryParams, headers, bodyFields, authType, authToken);
    const payload = {
      method: requestMethod,
      url: trimmedUrl,
      items,
      env_profile: selectedEnv === 'none' ? null : selectedEnv,
      pre_request_script: preRequestScript || null,
      post_response_script: postResponseScript || null
    };

    try {
      const resp = await invoke<ResponseDto>('send_request', { payload });
      setResponse(resp);
      
      const dbReports = await invoke<ReportDto[]>('get_reports');
      setReports(dbReports);
    } catch (e) {
      setErrorText(String(e).replace(/^Error:\s*/, ''));
    } finally {
      setIsLoading(false);
    }
  };

  const handleUrlChange = (val: string) => {
    if (val.trim().toLowerCase().startsWith('curl')) {
      const parsed = parseCurlCommand(val);
      if (parsed) {
        setRequestMethod(parsed.method);
        setRequestUrl(parsed.url);
        if (parsed.headers.length > 0) {
          setHeaders(parsed.headers.map(h => ({ key: h.key, value: h.value, enabled: true })));
        }
        if (parsed.bodyFields.length > 0) {
          setBodyFields(parsed.bodyFields.map(f => ({ key: f.key, value: f.value, type: 'string', enabled: true })));
        }
        if (parsed.headers.length > 0) {
          setRequestTab('headers');
        } else if (parsed.bodyFields.length > 0) {
          setRequestTab('body');
        }
        return;
      }
    }
    setRequestUrl(val);
  };

  const handleCopyAsCurl = () => {
    const curl = generateCurlCommand(requestMethod, requestUrl, headers, bodyFields);
    navigator.clipboard.writeText(curl);
    setCopiedCurl(true);
    setTimeout(() => {
      setCopiedCurl(false);
    }, 2000);
  };

  // Run Stored Test Case
  const handleRunTestCase = async () => {
    if (!selectedTestCase) return;
    setIsRunningTest(true);
    setRunReport(null);
    try {
      const report = await invoke<TestReport>('run_test_case', {
        payload: {
          suite: selectedTestCase.suite,
          name: selectedTestCase.name
        }
      });
      setRunReport(report);
    } catch (e) {
      alert(`Failed to execute test case: ${e}`);
    } finally {
      setIsRunningTest(false);
    }
  };

  // Table row modifiers
  const updateQueryParam = <K extends keyof ParamRow>(index: number, field: K, value: ParamRow[K]) => {
    const updated = [...queryParams];
    updated[index] = { ...updated[index], [field]: value };
    if (index === updated.length - 1 && updated[index].key.trim()) {
      updated.push({ key: '', value: '', enabled: true });
    }
    setQueryParams(updated);
  };

  const deleteQueryParam = (index: number) => {
    if (queryParams.length <= 1) {
      setQueryParams([{ key: '', value: '', enabled: true }]);
    } else {
      setQueryParams(queryParams.filter((_, i) => i !== index));
    }
  };

  const updateHeader = <K extends keyof ParamRow>(index: number, field: K, value: ParamRow[K]) => {
    const updated = [...headers];
    updated[index] = { ...updated[index], [field]: value };
    if (index === updated.length - 1 && updated[index].key.trim()) {
      updated.push({ key: '', value: '', enabled: true });
    }
    setHeaders(updated);
  };

  const deleteHeader = (index: number) => {
    if (headers.length <= 1) {
      setHeaders([{ key: '', value: '', enabled: true }]);
    } else {
      setHeaders(headers.filter((_, i) => i !== index));
    }
  };

  const updateBodyField = <K extends keyof BodyRow>(index: number, field: K, value: BodyRow[K]) => {
    const updated = [...bodyFields];
    updated[index] = { ...updated[index], [field]: value };
    if (index === updated.length - 1 && updated[index].key.trim()) {
      updated.push({ key: '', value: '', type: 'string', enabled: true });
    }
    setBodyFields(updated);
  };

  const deleteBodyField = (index: number) => {
    if (bodyFields.length <= 1) {
      setBodyFields([{ key: '', value: '', type: 'string', enabled: true }]);
    } else {
      setBodyFields(bodyFields.filter((_, i) => i !== index));
    }
  };

  const currentWorkspace = workspaces.find(w => w.name === activeWorkspaceName);

  // Filtered requests based on search query
  const filteredRequests = currentWorkspace
    ? currentWorkspace.requests.filter(r => 
        r.name.toLowerCase().includes(searchQuery.toLowerCase()) || 
        r.url.toLowerCase().includes(searchQuery.toLowerCase())
      )
    : [];

  const requestTree = useMemo(() => {
    const filteredEmpty = searchQuery 
      ? customEmptyFolders.filter(f => f.toLowerCase().includes(searchQuery.toLowerCase()))
      : customEmptyFolders;
    return buildRequestTree(filteredRequests, filteredEmpty);
  }, [filteredRequests, customEmptyFolders, searchQuery]);

  const suiteTree = useMemo(() => buildSuiteTree(testCases), [testCases]);

  const testCasesSummary = useMemo(() => {
    let passed = 0;
    let failed = 0;
    let untested = 0;

    for (const tc of testCases) {
      const run = testRuns.find(r => r.suite === tc.suite && r.case_name === tc.name);
      if (!run) {
        untested++;
      } else if (run.passed) {
        passed++;
      } else {
        failed++;
      }
    }

    return { passed, failed, untested, total: testCases.length };
  }, [testCases, testRuns]);

  const latestReportByRequest = useMemo(() => {
    const byRequest = new Map<string, ReportMeta>();
    for (const report of reports) {
      const meta = reportMeta(report);
      const exactKey = historyKey(meta.method, meta.url);
      if (exactKey && !byRequest.has(exactKey)) {
        byRequest.set(exactKey, meta);
      }
      const finalKey = historyKey(meta.method, meta.final_url);
      if (finalKey && !byRequest.has(finalKey)) {
        byRequest.set(finalKey, meta);
      }
    }
    return byRequest;
  }, [reports]);

  const renderRequestTree = (nodes: RequestTreeNode[], depth: number = 0): ReactNode[] => {
    const elements: ReactNode[] = [];
    
    for (const node of nodes) {
      const pathKey = node.path;
      const isExpanded = expandedRequestFolders[pathKey] !== false;
      
      const getRecursiveReqCount = (n: RequestTreeNode): number => {
        let count = n.isFolder ? 0 : 1;
        for (const child of n.children) {
          count += getRecursiveReqCount(child);
        }
        return count;
      };
      
      if (node.isFolder) {
        const totalReqs = getRecursiveReqCount(node);
        
        elements.push(
          <div key={pathKey} style={{ display: 'flex', flexDirection: 'column' }}>
            <div 
              className={`request-tree-item ${dragOverFolder === pathKey ? 'drag-over-folder' : ''}`}
              style={{ 
                display: 'flex', 
                justifyContent: 'space-between', 
                alignItems: 'center', 
                paddingLeft: `${depth * 12 + 6}px`, 
                fontWeight: '600',
                height: '32px',
                backgroundColor: dragOverFolder === pathKey ? 'var(--accent-light)' : undefined,
                border: dragOverFolder === pathKey ? '1px dashed var(--accent-color)' : undefined,
              }}
              onClick={() => {
                setExpandedRequestFolders(prev => ({ ...prev, [pathKey]: !isExpanded }));
              }}
              onDragOver={(e) => {
                e.preventDefault();
                e.stopPropagation();
                e.dataTransfer.dropEffect = 'move';
              }}
              onDragEnter={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setDragOverFolder(pathKey);
              }}
              onDragLeave={(e) => {
                e.preventDefault();
                e.stopPropagation();
                if (dragOverFolder === pathKey) {
                  setDragOverFolder(null);
                }
              }}
              onDrop={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setDragOverFolder(null);
                const reqId = e.dataTransfer.getData("requestId");
                if (reqId) {
                  handleMoveRequestToFolder(reqId, node.path);
                }
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', minWidth: 0, flex: 1 }}>
                <button 
                  className="icon-btn" 
                  style={{ padding: '2px', cursor: 'pointer' }}
                  onClick={(e) => {
                    e.stopPropagation();
                    setExpandedRequestFolders(prev => ({ ...prev, [pathKey]: !isExpanded }));
                  }}
                >
                  {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </button>
                <Folder size={14} style={{ color: 'var(--accent-hover)' }} />
                <span className="request-name" style={{ fontSize: '13px', cursor: 'pointer' }} title={node.name}>
                  {node.name}
                </span>
                <span style={{ fontSize: '11px', color: 'var(--text-muted)', fontWeight: 'normal' }}>
                  ({totalReqs})
                </span>
              </div>
              
              <div style={{ display: 'flex', gap: '4px', marginRight: '6px' }} onClick={e => e.stopPropagation()}>
                <button 
                  className="icon-btn"
                  onClick={() => handleCreateRequestInFolder(node.path)}
                  title="New Request in Folder"
                  style={{ padding: '4px' }}
                >
                  <Plus size={12} />
                </button>
                <button 
                  className="icon-btn delete-btn-hover"
                  onClick={() => handleDeleteRequestFolder(node.path)}
                  title="Delete Folder"
                  style={{ padding: '4px' }}
                >
                  <Trash size={12} style={{ color: 'var(--color-delete)' }} />
                </button>
              </div>
            </div>

            {isExpanded && (
              <div style={{ display: 'flex', flexDirection: 'column' }}>
                {renderRequestTree(node.children, depth + 1)}
              </div>
            )}
          </div>
        );
      } else {
        const req = node.requests[0];
        if (req) {
          const isActive = activeRequest && activeRequest.id === req.id;
          const lastRun = latestReportByRequest.get(historyKey(req.method, req.url));
          elements.push(
            <div 
              key={req.id || req.name} 
              className={`request-tree-item ${isActive ? 'active' : ''}`}
              onClick={() => loadRequestDetails(req)}
              draggable={true}
              onDragStart={(e) => {
                if (req.id) {
                  e.dataTransfer.setData("requestId", req.id);
                  e.dataTransfer.effectAllowed = 'move';
                }
              }}
              style={{ 
                display: 'flex', 
                justifyContent: 'space-between', 
                alignItems: 'center', 
                paddingLeft: `${depth * 12 + 18}px`,
                height: '32px',
                cursor: 'grab'
              }}
            >
              <div className="request-tree-left" style={{ minWidth: 0, flex: 1 }}>
                <span className={`method-tag method-${req.method.toLowerCase()}`} style={{ fontSize: '9px', minWidth: '38px', padding: '1px 3px' }}>
                  {req.method}
                </span>
                <span className="request-name" style={{ fontSize: '12px' }} title={req.name}>{req.name.split('/').pop()?.trim()}</span>
              </div>
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                {lastRun && (
                  <div className="request-last-run" style={{ fontSize: '11px' }}>
                    <span className={lastRun.status && lastRun.status < 400 ? 'history-status-ok' : 'history-status-error'}>
                      {lastRun.status || 'ERR'}
                    </span>
                  </div>
                )}
                <button 
                  className="icon-btn delete-btn-hover" 
                  onClick={(e) => {
                    e.stopPropagation();
                    if (req.id) handleDeleteRequest(req.id, req.name);
                  }}
                  title="Delete Request"
                  style={{ padding: '4px' }}
                >
                  <Trash size={12} style={{ color: 'var(--color-delete)' }} />
                </button>
              </div>
            </div>
          );
        }
      }
    }
    
    return elements;
  };

  const renderSuiteTree = (nodes: SuiteTreeNode[], depth: number = 0): ReactNode[] => {
    const elements: ReactNode[] = [];
    
    for (const node of nodes) {
      const pathKey = node.path;
      const isExpanded = expandedFolders[pathKey] !== false;
      const hasChildren = node.children.length > 0;
      
      const getRecursiveCases = (n: SuiteTreeNode): StoredTestCase[] => {
        let result = [...n.cases];
        for (const child of n.children) {
          result = result.concat(getRecursiveCases(child));
        }
        return result;
      };
      
      const recursiveCases = getRecursiveCases(node);
      const totalCases = recursiveCases.length;

      if (hasChildren || node.cases.length > 0) {
        const isSuiteActive = selectedSuitePath === node.path;
        
        elements.push(
          <div key={pathKey} style={{ display: 'flex', flexDirection: 'column' }}>
            <div 
              className={`request-tree-item ${isSuiteActive ? 'active' : ''}`}
              style={{ 
                display: 'flex', 
                justifyContent: 'space-between', 
                alignItems: 'center', 
                paddingLeft: `${depth * 12 + 6}px`, 
                fontWeight: '600',
                height: '32px'
              }}
              onClick={() => {
                setSelectedSuitePath(node.path);
                setSelectedTestCase(null);
                setSuiteReport(null);
              }}
            >
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', minWidth: 0, flex: 1 }}>
                <button 
                  className="icon-btn" 
                  style={{ padding: '2px', cursor: 'pointer' }}
                  onClick={(e) => {
                    e.stopPropagation();
                    setExpandedFolders(prev => ({ ...prev, [pathKey]: !isExpanded }));
                  }}
                >
                  {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </button>
                <Folder size={14} style={{ color: 'var(--accent-hover)' }} />
                <span className="request-name" style={{ fontSize: '13px', cursor: 'pointer' }} title={node.name}>
                  {node.name}
                </span>
                <span style={{ fontSize: '11px', color: 'var(--text-muted)', fontWeight: 'normal' }}>
                  ({totalCases})
                </span>
              </div>
              
              <div style={{ display: 'flex', gap: '2px' }} onClick={e => e.stopPropagation()}>
                <button 
                  className="icon-btn"
                  onClick={() => {
                    setSelectedSuitePath(node.path);
                    setSelectedTestCase(null);
                    handleRunTestSuite(node.path);
                  }}
                  title="Run Suite / Folder"
                >
                  <Play size={12} fill="currentColor" style={{ color: 'var(--color-get)' }} />
                </button>
                <button 
                  className="icon-btn"
                  onClick={() => handleDeleteTestSuite(node.path)}
                  title="Delete Suite / Folder"
                >
                  <Trash size={12} style={{ color: 'var(--color-delete)' }} />
                </button>
              </div>
            </div>

            {isExpanded && (
              <div style={{ display: 'flex', flexDirection: 'column' }}>
                {renderSuiteTree(node.children, depth + 1)}
                {node.cases.map((tc, idx) => {
                  const isActive = selectedTestCase && selectedTestCase.suite === tc.suite && selectedTestCase.name === tc.name;
                  return (
                    <div 
                      key={idx}
                      className={`request-tree-item ${isActive ? 'active' : ''}`}
                      onClick={() => {
                        setSelectedTestCase(tc);
                        setSelectedSuitePath(null);
                        setRunReport(null);
                      }}
                      style={{ 
                        display: 'flex', 
                        justifyContent: 'space-between', 
                        alignItems: 'center', 
                        paddingLeft: `${(depth + 1) * 12 + 18}px`,
                        height: '32px'
                      }}
                    >
                      <div className="request-tree-left" style={{ minWidth: 0, flex: 1 }}>
                        <span className={`method-tag method-${tc.method.toLowerCase()}`} style={{ fontSize: '9px', minWidth: '38px', padding: '1px 3px' }}>
                          {tc.method}
                        </span>
                        <span className="request-name" style={{ fontSize: '12px' }} title={tc.name}>{tc.name}</span>
                      </div>
                      <button 
                        className="icon-btn delete-btn-hover" 
                        onClick={(e) => {
                          e.stopPropagation();
                          handleDeleteTestCase(tc.suite, tc.name);
                        }}
                        title="Delete Test Case"
                        style={{ padding: '4px' }}
                      >
                        <Trash size={12} style={{ color: 'var(--color-delete)' }} />
                      </button>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      }
    }
    
    return elements;
  };

  return (
    <div className="app-shell">
      {/* Top Toolbar */}
      <header className="toolbar">
        <div className="toolbar-left">
          <div className="logo-container">
            <Globe size={18} className="logo-icon" />
            <span>ZapReq</span>
          </div>

          <div className="workspace-select-wrapper">
            <span className="select-label">Workspace</span>
            <select 
              className="toolbar-select"
              value={activeWorkspaceName}
              onChange={(e) => {
                if (e.target.value === 'new') {
                  setShowCreateWorkspace(true);
                } else {
                  setActiveWorkspaceName(e.target.value);
                  setActiveRequest(null);
                }
              }}
            >
              {workspaces.map(w => (
                <option key={w.name} value={w.name}>{w.name}</option>
              ))}
              <option value="new">+ Create Workspace...</option>
            </select>
            
            {activeWorkspaceName && (
              <button 
                className="icon-btn" 
                onClick={() => {
                  setNewWsRenameName(activeWorkspaceName);
                  setShowRenameWorkspace(true);
                }}
                title="Rename Workspace"
                style={{ padding: '6px' }}
              >
                <Edit size={14} />
              </button>
            )}
          </div>

          <div className="env-select-wrapper">
            <span className="select-label">Env</span>
            <select 
              className="toolbar-select"
              value={selectedEnv}
              onChange={(e) => setSelectedEnv(e.target.value)}
            >
              <option value="none">No Environment</option>
              {environments.map(env => (
                <option key={env} value={env}>{env}</option>
              ))}
            </select>
          </div>
        </div>

        <div className="toolbar-right">
          <button 
            className="btn btn-secondary" 
            onClick={() => setShowImportDialog(true)} 
            style={{ padding: '6px 12px', fontSize: '13px', display: 'flex', alignItems: 'center', gap: '4px' }}
          >
            <span>Import</span>
          </button>
          <button 
            className="btn btn-secondary" 
            onClick={() => {
              setExportFilePath(prev => prev || defaultExportFilename(activeWorkspaceName, exportFormat));
              setShowExportDialog(true);
            }} 
            style={{ padding: '6px 12px', fontSize: '13px', display: 'flex', alignItems: 'center', gap: '4px' }}
          >
            <span>Export</span>
          </button>
          <button className="icon-btn" onClick={() => loadInitialData()} title="Refresh Data">
            <RefreshCw size={16} />
          </button>
          <button className="icon-btn" onClick={() => setShowSettings(true)} title="App Information">
            <HelpCircle size={16} />
          </button>
        </div>
      </header>

      {/* Main Workspace Body */}
      <main className="workbench" ref={containerRef}>
        
        {/* Far Left Activity Bar */}
        <nav className="activity-bar">
          <button 
            className={`activity-btn ${activeView === 'collections' ? 'active' : ''}`}
            onClick={() => setActiveView('collections')}
            title="Collections & Requests"
          >
            <Plus size={20} />
          </button>
          
          <button 
            className={`activity-btn ${activeView === 'tests' ? 'active' : ''}`}
            onClick={() => setActiveView('tests')}
            title="Regression Tests Runner"
          >
            <ShieldCheck size={20} />
          </button>

          <button 
            className={`activity-btn ${activeView === 'reports' ? 'active' : ''}`}
            onClick={() => setActiveView('reports')}
            title="Execution History Reports"
          >
            <Database size={20} />
          </button>
        </nav>

        {/* 1. COLLECTIONS VIEW VIEWPORTS */}
        {activeView === 'collections' && (
          <>
            {/* Left Sidebar */}
            <section className="sidebar" style={{ width: `${sidebarWidth}px` }}>
              <div className="sidebar-header">
                <span className="sidebar-title">Collections</span>
                <div style={{ display: 'flex', gap: '4px' }}>
                  <button className="icon-btn" onClick={handleCreateRequestFolder} title="New Folder">
                    <FolderPlus size={16} />
                  </button>
                  <button className="icon-btn" onClick={handleCreateRequest} title="New Request">
                    <Plus size={16} />
                  </button>
                </div>
              </div>

              <div className="sidebar-search">
                <div className="search-input-wrapper">
                  <Search size={14} className="search-icon" />
                  <input 
                    type="text" 
                    placeholder="Filter requests..." 
                    className="sidebar-search-input"
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                  />
                </div>
              </div>

              <div 
                className="request-tree-container"
                style={{
                  border: (dragOverRoot && !dragOverFolder) ? '1px dashed var(--accent-color)' : '1px solid transparent',
                  borderRadius: '4px',
                  margin: '4px',
                  backgroundColor: (dragOverRoot && !dragOverFolder) ? 'rgba(79, 70, 229, 0.05)' : undefined,
                  transition: 'all 0.15s ease'
                }}
                onDragOver={(e) => {
                  e.preventDefault();
                  e.dataTransfer.dropEffect = 'move';
                }}
                onDragEnter={(e) => {
                  e.preventDefault();
                  setDragOverRoot(true);
                }}
                onDragLeave={(e) => {
                  const rect = e.currentTarget.getBoundingClientRect();
                  const x = e.clientX;
                  const y = e.clientY;
                  if (x < rect.left || x >= rect.right || y < rect.top || y >= rect.bottom) {
                    setDragOverRoot(false);
                  }
                }}
                onDrop={(e) => {
                  e.preventDefault();
                  setDragOverRoot(false);
                  setDragOverFolder(null);
                  const reqId = e.dataTransfer.getData("requestId");
                  if (reqId) {
                    handleMoveRequestToFolder(reqId, "");
                  }
                }}
              >
                {requestTree.length === 0 ? (
                  <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '13px' }}>
                    No requests found
                  </div>
                ) : (
                  renderRequestTree(requestTree)
                )}
              </div>

              <div className="sidebar-footer">
                <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                  <Database size={12} />
                  <span>{currentWorkspace?.requests.length || 0} Saved Requests</span>
                </div>
              </div>
            </section>

            <div className={`resize-divider ${isResizingSidebar ? 'dragging' : ''}`} onMouseDown={handleMouseDownSidebar} />

            {/* Center Request Editor */}
            <section className="request-pane">
              {activeRequest ? (
                <>
                  <div className="request-header">
                    <div className="request-title-row">
                      <input 
                        type="text" 
                        className="request-title-input" 
                        value={requestName}
                        onChange={(e) => setRequestName(e.target.value)}
                        onBlur={handleSaveRequest}
                        style={{ flex: 1, minWidth: 0 }}
                      />
                      <div style={{ display: 'flex', gap: '8px', flexShrink: 0, alignItems: 'center' }}>
                        <button 
                          className="btn btn-secondary" 
                          onClick={() => {
                            setTestSuiteName("");
                            setTestCaseName(requestName);
                            setExpectStatus(response ? String(response.status) : "200");
                            setExpectMaxTime(response ? String(response.elapsed_ms || 500) : "500");
                            setExpectContains("");
                            setSelectedRequestForTestCaseId(activeRequest?.id || "custom");
                            setTcRequestMethod(requestMethod);
                            setTcRequestUrl(requestUrl);
                            setTcRequestItems(buildRequestItems(queryParams, headers, bodyFields, authType, authToken));
                            setShowAddTestDialog(true);
                          }} 
                          style={{ display: 'flex', alignItems: 'center', gap: '4px', whiteSpace: 'nowrap' }}
                          title="Add this request to a regression test suite"
                        >
                          <ShieldCheck size={14} />
                          <span>Create Test Case</span>
                        </button>
                        <button className="btn btn-secondary" onClick={handleSaveRequest} style={{ whiteSpace: 'nowrap' }}>
                          Save Changes
                        </button>
                      </div>
                    </div>

                    <div className="request-url-row">
                      <select 
                        className="method-select"
                        value={requestMethod}
                        onChange={(e) => setRequestMethod(e.target.value)}
                      >
                        <option value="GET">GET</option>
                        <option value="POST">POST</option>
                        <option value="PUT">PUT</option>
                        <option value="DELETE">DELETE</option>
                        <option value="PATCH">PATCH</option>
                        <option value="OPTIONS">OPTIONS</option>
                        <option value="HEAD">HEAD</option>
                      </select>

                      <div className="url-input-wrapper">
                        <input 
                          type="text" 
                          className="url-input" 
                          placeholder="https://api.example.com/endpoint"
                          value={requestUrl}
                          onChange={(e) => handleUrlChange(e.target.value)}
                        />
                      </div>

                      <button 
                        className="send-btn" 
                        onClick={handleSendRequest}
                        disabled={isLoading || !requestUrl.trim()}
                      >
                        {isLoading ? (
                          <>
                            <RefreshCw size={14} className="animate-spin" />
                            <span>Sending...</span>
                          </>
                        ) : (
                          <>
                            <Play size={14} fill="white" />
                            <span>Send</span>
                          </>
                        )}
                      </button>

                      <button 
                        className="btn btn-secondary" 
                        onClick={handleCopyAsCurl}
                        title="Copy as Curl Command"
                        style={{ display: 'flex', alignItems: 'center', gap: '4px', whiteSpace: 'nowrap', boxSizing: 'border-box' }}
                      >
                        <Copy size={14} />
                        <span>{copiedCurl ? "Copied!" : "Curl"}</span>
                      </button>
                    </div>
                  </div>

                  <div className="tabs-row">
                    <button className={`tab-btn ${requestTab === 'params' ? 'active' : ''}`} onClick={() => setRequestTab('params')}>Params</button>
                    <button className={`tab-btn ${requestTab === 'headers' ? 'active' : ''}`} onClick={() => setRequestTab('headers')}>Headers</button>
                    <button className={`tab-btn ${requestTab === 'auth' ? 'active' : ''}`} onClick={() => setRequestTab('auth')}>Auth</button>
                    <button className={`tab-btn ${requestTab === 'body' ? 'active' : ''}`} onClick={() => setRequestTab('body')}>Body</button>
                    <button className={`tab-btn ${requestTab === 'pre-request' ? 'active' : ''}`} onClick={() => setRequestTab('pre-request')}>Pre-request</button>
                    <button className={`tab-btn ${requestTab === 'tests' ? 'active' : ''}`} onClick={() => setRequestTab('tests')}>Tests</button>
                  </div>

                  <div className="tab-content">
                    {requestTab === 'params' && (
                      <div className="kv-table-container">
                        <div className="kv-header">
                          <span>Parameter Key</span>
                          <span>Value</span>
                          <span></span>
                        </div>
                        {queryParams.map((row, idx) => (
                          <div key={idx} className="kv-row">
                            <input type="text" placeholder="key" className="kv-input" value={row.key} onChange={(e) => updateQueryParam(idx, 'key', e.target.value)} />
                            <input type="text" placeholder="value" className="kv-input" value={row.value} onChange={(e) => updateQueryParam(idx, 'value', e.target.value)} />
                            <button className="icon-btn" onClick={() => deleteQueryParam(idx)}><Trash size={14} /></button>
                          </div>
                        ))}
                      </div>
                    )}

                    {requestTab === 'headers' && (
                      <div className="kv-table-container">
                        <div className="kv-header">
                          <span>Header Key</span>
                          <span>Value</span>
                          <span></span>
                        </div>
                        {headers.map((row, idx) => (
                          <div key={idx} className="kv-row">
                            <input type="text" placeholder="key" className="kv-input" value={row.key} onChange={(e) => updateHeader(idx, 'key', e.target.value)} />
                            <input type="text" placeholder="value" className="kv-input" value={row.value} onChange={(e) => updateHeader(idx, 'value', e.target.value)} />
                            <button className="icon-btn" onClick={() => deleteHeader(idx)}><Trash size={14} /></button>
                          </div>
                        ))}
                      </div>
                    )}

                    {requestTab === 'auth' && (
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                        <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                          <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Auth Type</label>
                          <select className="toolbar-select" value={authType} onChange={(e) => setAuthType(e.target.value)} style={{ width: '220px' }}>
                            <option value="none">No Authentication</option>
                            <option value="bearer">Bearer Token</option>
                          </select>
                        </div>
                        {authType === 'bearer' && (
                          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                            <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Token</label>
                            <input type="password" className="kv-input" placeholder="secret_token_values" value={authToken} onChange={(e) => setAuthToken(e.target.value)} style={{ width: '360px' }} />
                          </div>
                        )}
                      </div>
                    )}

                    {requestTab === 'body' && (
                      <div className="kv-table-container">
                        <div className="kv-header" style={{ gridTemplateColumns: '1.2fr 1.5fr 80px 40px' }}>
                          <span>Field Key</span>
                          <span>Value</span>
                          <span>Type</span>
                          <span></span>
                        </div>
                        {bodyFields.map((row, idx) => (
                          <div key={idx} className="kv-row" style={{ gridTemplateColumns: '1.2fr 1.5fr 80px 40px' }}>
                            <input type="text" placeholder="key" className="kv-input" value={row.key} onChange={(e) => updateBodyField(idx, 'key', e.target.value)} />
                            <input type="text" placeholder="value" className="kv-input" value={row.value} onChange={(e) => updateBodyField(idx, 'value', e.target.value)} />
                            <select className="toolbar-select" value={row.type} onChange={(e) => updateBodyField(idx, 'type', e.target.value as BodyRow['type'])} style={{ minWidth: 'auto', padding: '6px 8px' }}>
                              <option value="string">Text</option>
                              <option value="json">JSON</option>
                            </select>
                            <button className="icon-btn" onClick={() => deleteBodyField(idx)}><Trash size={14} /></button>
                          </div>
                        ))}
                      </div>
                    )}

                    {requestTab === 'pre-request' && (
                      <div style={{ display: 'flex', flexDirection: 'column', height: '100%', gap: '8px' }}>
                        <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Write JavaScript to run before sending the request. E.g., <code>pm.variables.set("token", "val");</code></span>
                        <textarea 
                          className="response-body-pre" 
                          value={preRequestScript} 
                          onChange={(e) => setPreRequestScript(e.target.value)} 
                          placeholder="// Pre-request JavaScript code..."
                          style={{ width: '100%', minHeight: '220px', fontFamily: 'monospace', fontSize: '13px', backgroundColor: 'var(--bg-secondary)', color: 'var(--text-primary)', border: '1px solid var(--border-color)', borderRadius: '4px', padding: '12px', resize: 'vertical' }}
                          onBlur={handleSaveRequest}
                        />
                      </div>
                    )}

                    {requestTab === 'tests' && (
                      <div style={{ display: 'flex', flexDirection: 'column', height: '100%', gap: '8px' }}>
                        <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Write JavaScript to execute assertions after receiving the response. E.g., <code>{"pm.test(\"Status is 200\", () => pm.expect(pm.response.code).toBe(200));"}</code></span>
                        <textarea 
                          className="response-body-pre" 
                          value={postResponseScript} 
                          onChange={(e) => setPostResponseScript(e.target.value)} 
                          placeholder="// Test / Post-response JavaScript code..."
                          style={{ width: '100%', minHeight: '220px', fontFamily: 'monospace', fontSize: '13px', backgroundColor: 'var(--bg-secondary)', color: 'var(--text-primary)', border: '1px solid var(--border-color)', borderRadius: '4px', padding: '12px', resize: 'vertical' }}
                          onBlur={handleSaveRequest}
                        />
                      </div>
                    )}
                  </div>
                </>
              ) : (
                <div className="response-idle-state" style={{ backgroundColor: 'var(--bg-primary)' }}>
                  <div style={{ padding: '24px', backgroundColor: 'var(--bg-secondary)', borderRadius: '50%', border: '1px solid var(--border-color)' }}>
                    <Terminal size={32} className="idle-icon" />
                  </div>
                  <h3 className="idle-title">Select a request or create a new one</h3>
                  <p className="idle-text">Use the sidebar to pick an existing REST API request, or click the + button to create a new spec.</p>
                  <button className="btn btn-primary" onClick={handleCreateRequest}>Create New Request</button>
                </div>
              )}
            </section>

            <div className={`resize-divider ${isResizingResponse ? 'dragging' : ''}`} onMouseDown={handleMouseDownResponse} />

            {/* Right Response Panel */}
            <section className="response-pane" style={{ width: `${responseWidth}px` }}>
              {isLoading ? (
                <div className="response-idle-state">
                  <RefreshCw size={32} className="animate-spin text-indigo-500" style={{ color: 'var(--accent-color)' }} />
                  <h3 className="idle-title">Executing Request</h3>
                  <p className="idle-text">Waiting for backend server response trace...</p>
                </div>
              ) : errorText ? (
                <div className="response-idle-state" style={{ color: '#ef4444' }}>
                  <AlertCircle size={32} />
                  <h3 className="idle-title" style={{ color: '#ef4444' }}>Request Error</h3>
                  <p className="idle-text" style={{ wordBreak: 'break-all' }}>{errorText}</p>
                </div>
              ) : response ? (
                <>
                  <div className="response-header">
                    <div className="response-summary">
                      <div className="summary-item">
                        <span className={`summary-value ${response.status < 300 ? 'status-success' : response.status < 400 ? 'status-redirect' : 'status-error'}`}>
                          {response.status} {response.reason}
                        </span>
                        <span className="summary-label">Status</span>
                      </div>
                      <div className="summary-item">
                        <span className="summary-value" style={{ color: 'var(--color-put)' }}>{response.elapsed_label}</span>
                        <span className="summary-label">Time</span>
                      </div>
                      <div className="summary-item">
                        <span className="summary-value">{response.size_label}</span>
                        <span className="summary-label">Size</span>
                      </div>
                    </div>
                    <span className="response-content-type">{response.content_type || 'unknown content'}</span>
                  </div>

                  <div className="tabs-row">
                    <button className={`tab-btn ${responseTab === 'body' ? 'active' : ''}`} onClick={() => setResponseTab('body')}>Body</button>
                    <button className={`tab-btn ${responseTab === 'headers' ? 'active' : ''}`} onClick={() => setResponseTab('headers')}>Headers</button>
                    <button className={`tab-btn ${responseTab === 'raw' ? 'active' : ''}`} onClick={() => setResponseTab('raw')}>Raw</button>
                    {response.test_results && response.test_results.length > 0 && (
                      <button className={`tab-btn ${responseTab === 'tests' ? 'active' : ''}`} onClick={() => setResponseTab('tests')}>Test Results</button>
                    )}
                  </div>

                  <div className="tab-content" style={{ padding: 0, display: 'flex', flexDirection: 'column' }}>
                    {responseTab === 'body' && (
                      <div className="response-body-wrapper">
                        {isHtmlResponse(response) && response.status >= 400 && (
                          <div className="response-error-banner">
                            <AlertCircle size={16} />
                            <span>The server returned an HTML error page. ZapReq is showing the readable text below; use Raw to inspect the original markup.</span>
                          </div>
                        )}
                        {isJsonResponse(response) ? (
                          <JsonCode source={formatResponseBody(response)} />
                        ) : (
                          <pre className="response-body-pre"><code>{formatResponseBody(response)}</code></pre>
                        )}
                      </div>
                    )}
                    {responseTab === 'raw' && (
                      <div className="response-body-wrapper">
                        <pre className="response-body-pre"><code>{response.body}</code></pre>
                      </div>
                    )}
                    {responseTab === 'headers' && (
                      <div style={{ padding: '20px', overflowY: 'auto' }}>
                        <table className="headers-table">
                          <thead>
                            <tr>
                              <th>Header Key</th>
                              <th>Value</th>
                            </tr>
                          </thead>
                          <tbody>
                            {response.headers.map((h, i) => (
                              <tr key={i}>
                                <td style={{ fontWeight: '500', color: 'var(--text-secondary)' }}>{h.key}</td>
                                <td>{h.value}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    )}
                    {responseTab === 'tests' && response.test_results && (
                      <div style={{ padding: '20px', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                        <h4 style={{ fontSize: '11px', textTransform: 'uppercase', color: 'var(--text-muted)', fontWeight: '600', marginBottom: '8px' }}>JavaScript Assertions Checklist</h4>
                        {response.test_results.map((res, i) => {
                          const isPass = res.startsWith("PASS:");
                          const cleanText = res.substring(5).trim();
                          return (
                            <div key={i} className={`assertion-item ${isPass ? 'passed' : 'failed'}`}>
                              <div style={{ display: 'flex', gap: '8px', alignItems: 'center' }}>
                                {isPass ? (
                                  <CheckCircle2 size={16} style={{ color: 'var(--color-get)' }} />
                                ) : (
                                  <AlertCircle size={16} style={{ color: 'var(--color-delete)' }} />
                                )}
                                <span style={{ fontSize: '13px', fontWeight: '500', color: 'var(--text-primary)' }}>{cleanText}</span>
                              </div>
                            </div>
                          );
                        })}
                      </div>
                    )}
                  </div>
                </>
              ) : (
                <div className="response-idle-state">
                  <div style={{ padding: '24px', backgroundColor: 'var(--bg-primary)', borderRadius: '50%', border: '1px solid var(--border-color)' }}>
                    <Play size={28} className="idle-icon" />
                  </div>
                  <h3 className="idle-title">Response panel is idle</h3>
                  <p className="idle-text font-normal">Send a request to inspect response body, headers, cookies, timing, and payload size.</p>
                </div>
              )}
            </section>
          </>
        )}

        {/* 2. REGRESSION TESTS RUNNER VIEWPORTS */}
        {activeView === 'tests' && (
          <>
            {/* Left Sidebar */}
            <section className="sidebar" style={{ width: `${sidebarWidth}px` }}>
              <div className="sidebar-header">
                <span className="sidebar-title">Test Suites</span>
                <button 
                  className="icon-btn" 
                  onClick={() => {
                    setTestSuiteName("");
                    setTestCaseName("");
                    setExpectStatus("200");
                    setExpectMaxTime("500");
                    setExpectContains("");
                    const defaultReqId = currentWorkspace?.requests[0]?.id || "custom";
                    setSelectedRequestForTestCaseId(defaultReqId);
                    if (defaultReqId !== "custom") {
                      const req = currentWorkspace?.requests.find(r => r.id === defaultReqId);
                      setTestCaseName(req ? req.name : "");
                    }
                    setTcRequestMethod("GET");
                    setTcRequestUrl("");
                    setTcRequestItems([]);
                    setShowAddTestDialog(true);
                  }} 
                  title="Create Test Case"
                >
                  <Plus size={16} />
                </button>
              </div>
              <div className="request-tree-container">
                <div 
                  className={`request-tree-item ${(!selectedTestCase && !selectedSuitePath) ? 'active' : ''}`}
                  style={{ display: 'flex', alignItems: 'center', gap: '6px', fontWeight: '600', height: '32px', marginBottom: '8px', paddingLeft: '6px' }}
                  onClick={() => {
                    setSelectedTestCase(null);
                    setSelectedSuitePath(null);
                    setSuiteReport(null);
                  }}
                >
                  <ShieldCheck size={14} style={{ color: 'var(--accent-hover)' }} />
                  <span className="request-name" style={{ fontSize: '13px' }}>Dashboard Overview</span>
                </div>

                {suiteTree.length === 0 ? (
                  <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '13px' }}>
                    No test cases saved in SQLite DB
                  </div>
                ) : (
                  renderSuiteTree(suiteTree)
                )}
              </div>
            </section>

            <div className={`resize-divider ${isResizingSidebar ? 'dragging' : ''}`} onMouseDown={handleMouseDownSidebar} />

            {/* Center Test Specification Panel */}
            <section className="request-pane">
              {selectedTestCase && (
                <>
                  <div className="request-header">
                    <div>
                      <span className="select-label" style={{ display: 'block', marginBottom: '4px' }}>Suite: {selectedTestCase.suite}</span>
                      <h2 style={{ fontSize: '18px', fontWeight: '600' }}>{selectedTestCase.name}</h2>
                    </div>

                    <div style={{ display: 'flex', gap: '8px', alignItems: 'center', padding: '10px 14px', backgroundColor: 'var(--bg-secondary)', borderRadius: '6px', border: '1px solid var(--border-color)', fontSize: '13px', color: 'var(--text-secondary)' }}>
                      <span className={`method-tag method-${selectedTestCase.method.toLowerCase()}`}>{selectedTestCase.method}</span>
                      <span style={{ fontFamily: 'monospace' }}>{selectedTestCase.url}</span>
                    </div>
                  </div>

                  <div className="tab-content" style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
                    <div>
                      <h3 style={{ fontSize: '14px', fontWeight: '600', marginBottom: '10px', color: 'var(--text-primary)' }}>Execution Items</h3>
                      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
                        {selectedTestCase.items.length === 0 ? (
                          <span style={{ fontSize: '12px', color: 'var(--text-muted)' }}>No arguments</span>
                        ) : (
                          selectedTestCase.items.map((it, i) => (
                            <span key={i} style={{ fontSize: '11px', fontFamily: 'monospace', padding: '4px 8px', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '4px' }}>{it}</span>
                          ))
                        )}
                      </div>
                    </div>

                    <div>
                      <h3 style={{ fontSize: '14px', fontWeight: '600', marginBottom: '10px', color: 'var(--text-primary)' }}>Target Expectations (Assertions)</h3>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '8px', fontSize: '13px' }}>
                        {selectedTestCase.expect_status && (
                          <div style={{ display: 'flex', justifyContent: 'space-between', padding: '8px', backgroundColor: 'var(--bg-secondary)', borderRadius: '6px' }}>
                            <span style={{ color: 'var(--text-secondary)' }}>Expect Status Code</span>
                            <span style={{ fontWeight: '600', color: 'var(--color-get)' }}>{selectedTestCase.expect_status}</span>
                          </div>
                        )}
                        {selectedTestCase.max_time_ms && (
                          <div style={{ display: 'flex', justifyContent: 'space-between', padding: '8px', backgroundColor: 'var(--bg-secondary)', borderRadius: '6px' }}>
                            <span style={{ color: 'var(--text-secondary)' }}>Expect Maximum Duration</span>
                            <span style={{ fontWeight: '600', color: 'var(--color-put)' }}>{selectedTestCase.max_time_ms} ms</span>
                          </div>
                        )}
                        {selectedTestCase.expect_headers && selectedTestCase.expect_headers.map((h, i) => (
                          <div key={i} style={{ display: 'flex', justifyContent: 'space-between', padding: '8px', backgroundColor: 'var(--bg-secondary)', borderRadius: '6px' }}>
                            <span style={{ color: 'var(--text-secondary)' }}>Expect Header HeaderName</span>
                            <span style={{ fontFamily: 'monospace' }}>{h}</span>
                          </div>
                        ))}
                        {selectedTestCase.expect_body_contains && selectedTestCase.expect_body_contains.map((c, i) => (
                          <div key={i} style={{ display: 'flex', justifyContent: 'space-between', padding: '8px', backgroundColor: 'var(--bg-secondary)', borderRadius: '6px' }}>
                            <span style={{ color: 'var(--text-secondary)' }}>Expect Body Contains String</span>
                            <span style={{ fontFamily: 'monospace', color: 'var(--color-post)' }}>"{c}"</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </>
              )}

              {selectedSuitePath && (
                <>
                  <div className="request-header">
                    <div>
                      <span className="select-label" style={{ display: 'block', marginBottom: '4px' }}>Regression Test Suite Folder</span>
                      <h2 style={{ fontSize: '18px', fontWeight: '600', display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <Folder size={20} style={{ color: 'var(--accent-hover)' }} />
                        <span>{selectedSuitePath}</span>
                      </h2>
                    </div>
                  </div>

                  <div className="tab-content" style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
                    <div>
                      <h3 style={{ fontSize: '14px', fontWeight: '600', marginBottom: '10px', color: 'var(--text-primary)' }}>Test Cases in Suite Folder</h3>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                        {testCases.filter(tc => tc.suite === selectedSuitePath || tc.suite.startsWith(selectedSuitePath + ' / ')).map((tc, i) => (
                          <div 
                            key={i} 
                            style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '10px 14px', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '6px', cursor: 'pointer' }}
                            onClick={() => {
                              setSelectedTestCase(tc);
                              setSelectedSuitePath(null);
                              setRunReport(null);
                            }}
                          >
                            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                              <span className={`method-tag method-${tc.method.toLowerCase()}`} style={{ fontSize: '10px', minWidth: '38px', padding: '1px 3px' }}>{tc.method}</span>
                              <span style={{ fontWeight: '500', fontSize: '13px' }}>{tc.name}</span>
                            </div>
                            <span style={{ fontFamily: 'monospace', fontSize: '12px', color: 'var(--text-muted)' }}>{tc.url}</span>
                          </div>
                        ))}
                      </div>
                    </div>
                  </div>
                </>
              )}

              {!selectedTestCase && !selectedSuitePath && (
                <>
                  <div className="request-header">
                    <div>
                      <span className="select-label" style={{ display: 'block', marginBottom: '4px' }}>Overview Dashboard</span>
                      <h2 style={{ fontSize: '18px', fontWeight: '600' }}>Regression Test Suites</h2>
                    </div>
                  </div>

                  <div className="tab-content" style={{ display: 'flex', flexDirection: 'column', gap: '20px', overflowY: 'auto' }}>
                    <div>
                      <h3 style={{ fontSize: '14px', fontWeight: '600', marginBottom: '12px', color: 'var(--text-primary)' }}>Test Case Status Summary</h3>
                      <TestCasesDonutChart 
                        passed={testCasesSummary.passed} 
                        failed={testCasesSummary.failed} 
                        untested={testCasesSummary.untested} 
                      />
                    </div>

                    <div>
                      <h3 style={{ fontSize: '14px', fontWeight: '600', marginBottom: '12px', color: 'var(--text-primary)' }}>Recent Runs Log Feed</h3>
                      <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                        {testRuns.length === 0 ? (
                          <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '13px', backgroundColor: 'var(--bg-secondary)', borderRadius: '6px', border: '1px solid var(--border-color)' }}>
                            No test case runs logged in history yet.
                          </div>
                        ) : (
                          testRuns.slice(0, 6).map((run) => (
                            <div 
                              key={run.id}
                              style={{ 
                                display: 'flex', 
                                justifyContent: 'space-between', 
                                alignItems: 'center', 
                                padding: '10px 14px', 
                                backgroundColor: 'var(--bg-secondary)', 
                                border: '1px solid var(--border-color)', 
                                borderRadius: '6px' 
                              }}
                            >
                              <div style={{ display: 'flex', flexDirection: 'column', gap: '2px', minWidth: 0 }}>
                                <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap' }}>
                                  <span style={{ fontSize: '11px', color: 'var(--text-muted)', fontFamily: 'monospace' }}>{run.suite}</span>
                                  <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>/</span>
                                  <span style={{ fontWeight: '500', fontSize: '13px', color: 'var(--text-primary)', textOverflow: 'ellipsis', overflow: 'hidden', whiteSpace: 'nowrap', maxWidth: '200px' }} title={run.case_name}>{run.case_name}</span>
                                </div>
                                <div style={{ display: 'flex', gap: '8px', fontSize: '11px', color: 'var(--text-muted)' }}>
                                  <span>{run.elapsed_ms} ms</span>
                                  <span>•</span>
                                  <span>Status: {run.status_code}</span>
                                  <span>•</span>
                                  <span>{new Date(run.created_at).toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' })}</span>
                                </div>
                              </div>
                              
                              <span 
                                className={`method-tag method-${run.passed ? 'get' : 'delete'}`}
                                style={{ 
                                  fontSize: '10px', 
                                  fontWeight: 'bold', 
                                  textTransform: 'uppercase', 
                                  padding: '2px 8px', 
                                  borderRadius: '4px',
                                  backgroundColor: run.passed ? 'rgba(16, 185, 129, 0.15)' : 'rgba(239, 68, 68, 0.15)',
                                  color: run.passed ? 'var(--color-get)' : 'var(--color-delete)'
                                }}
                              >
                                {run.passed ? 'Passed' : 'Failed'}
                              </span>
                            </div>
                          ))
                        )}
                      </div>
                    </div>
                  </div>
                </>
              )}
            </section>

            <div className={`resize-divider ${isResizingResponse ? 'dragging' : ''}`} onMouseDown={handleMouseDownResponse} />

            {/* Right Test Results Panel */}
            <section className="response-pane" style={{ width: `${responseWidth}px` }}>
              {selectedTestCase && (
                <>
                  <div className="response-header" style={{ justifyContent: 'center', padding: '24px' }}>
                    <button 
                      className="send-btn" 
                      onClick={handleRunTestCase} 
                      disabled={isRunningTest}
                      style={{ width: '100%', justifyContent: 'center', height: '42px' }}
                    >
                      {isRunningTest ? (
                        <>
                          <RefreshCw size={14} className="animate-spin" />
                          <span>Running Assertions...</span>
                        </>
                      ) : (
                        <>
                          <Play size={14} fill="white" />
                          <span>Execute Test Case</span>
                        </>
                      )}
                    </button>
                  </div>

                  <div className="tab-content" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                    {runReport ? (
                      <>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '12px', backgroundColor: runReport.passed ? 'rgba(16, 185, 129, 0.1)' : 'rgba(239, 68, 68, 0.1)', borderRadius: '8px', border: `1px solid ${runReport.passed ? 'var(--color-get)' : 'var(--color-delete)'}` }}>
                          {runReport.passed ? <CheckCircle2 size={18} style={{ color: 'var(--color-get)' }} /> : <AlertCircle size={18} style={{ color: 'var(--color-delete)' }} />}
                          <span style={{ fontWeight: '600', fontSize: '14px' }}>{runReport.passed ? "Test Passed Successfully" : "Expectation Assertion Failed"}</span>
                        </div>

                        <div style={{ display: 'flex', gap: '12px', fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '8px' }}>
                          <span>Duration: {runReport.elapsed_ms} ms</span>
                          <span>|</span>
                          <span>HTTP Status: {runReport.status}</span>
                        </div>

                        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                          <h4 style={{ fontSize: '11px', textTransform: 'uppercase', color: 'var(--text-muted)', fontWeight: '600' }}>Evaluation Checklist</h4>
                          {runReport.assertions.map((ass, i) => (
                            <div key={i} className={`assertion-item ${ass.passed ? 'passed' : 'failed'}`}>
                              <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
                                <span style={{ fontSize: '13px', fontWeight: '500', color: 'var(--text-primary)' }}>{ass.assertion}</span>
                                <span style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>{ass.details}</span>
                              </div>
                            </div>
                          ))}
                        </div>
                      </>
                    ) : (
                      <div className="response-idle-state" style={{ height: '300px' }}>
                        <Terminal size={24} className="idle-icon" />
                        <span className="idle-title" style={{ fontSize: '13px' }}>Runner is ready</span>
                        <p className="idle-text" style={{ fontSize: '12px' }}>Click the Execute button above to run real-time tests against this API specification.</p>
                      </div>
                    )}
                  </div>
                </>
              )}

              {selectedSuitePath && (
                <>
                  <div className="response-header" style={{ justifyContent: 'center', padding: '24px' }}>
                    <button 
                      className="send-btn" 
                      onClick={() => handleRunTestSuite(selectedSuitePath)} 
                      disabled={isRunningSuite}
                      style={{ width: '100%', justifyContent: 'center', height: '42px' }}
                    >
                      {isRunningSuite ? (
                        <>
                          <RefreshCw size={14} className="animate-spin" />
                          <span>Running Suite sweeps...</span>
                        </>
                      ) : (
                        <>
                          <Play size={14} fill="white" />
                          <span>Execute Suite Sweeps</span>
                        </>
                      )}
                    </button>
                  </div>

                  <div className="tab-content" style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                    {isRunningSuite && suiteProgress ? (
                      <div style={{ padding: '12px 0', display: 'flex', flexDirection: 'column', gap: '12px' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: 'var(--text-secondary)', fontSize: '13px' }}>
                          <RefreshCw size={14} className="animate-spin" />
                          <span>Running sequential assertions...</span>
                        </div>
                        <div style={{ padding: '12px', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '8px' }}>
                          <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: '8px', fontSize: '12px' }}>
                            <span style={{ fontWeight: '600', color: 'var(--text-primary)', textOverflow: 'ellipsis', overflow: 'hidden', whiteSpace: 'nowrap', maxWidth: '180px' }} title={suiteProgress.caseName}>{suiteProgress.caseName || 'Initializing...'}</span>
                            <span style={{ color: 'var(--text-muted)' }}>{suiteProgress.current} / {suiteProgress.total}</span>
                          </div>
                          <div style={{ width: '100%', height: '6px', backgroundColor: 'rgba(255,255,255,0.08)', borderRadius: '3px', overflow: 'hidden' }}>
                            <div style={{ width: `${suiteProgress.total > 0 ? (suiteProgress.current / suiteProgress.total) * 100 : 0}%`, height: '100%', backgroundColor: 'var(--accent-hover)', transition: 'width 0.1s ease' }} />
                          </div>
                        </div>
                      </div>
                    ) : suiteReport ? (
                      <>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '12px', backgroundColor: suiteReport.failed === 0 ? 'rgba(16, 185, 129, 0.1)' : 'rgba(239, 68, 68, 0.1)', borderRadius: '8px', border: `1px solid ${suiteReport.failed === 0 ? 'var(--color-get)' : 'var(--color-delete)'}` }}>
                          {suiteReport.failed === 0 ? <CheckCircle2 size={18} style={{ color: 'var(--color-get)' }} /> : <AlertCircle size={18} style={{ color: 'var(--color-delete)' }} />}
                          <span style={{ fontWeight: '600', fontSize: '14px' }}>
                            {suiteReport.failed === 0 ? "All Tests Passed" : `${suiteReport.failed} Assertions Failed`}
                          </span>
                        </div>

                        <div style={{ display: 'flex', gap: '12px', fontSize: '11px', color: 'var(--text-secondary)', marginBottom: '8px' }}>
                          <span>Passed: {suiteReport.passed}</span>
                          <span>|</span>
                          <span>Failed: {suiteReport.failed}</span>
                        </div>

                        <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                          <h4 style={{ fontSize: '11px', textTransform: 'uppercase', color: 'var(--text-muted)', fontWeight: '600' }}>Suite Cases</h4>
                          {suiteReport.cases.map((c, i) => (
                            <div 
                              key={i} 
                              className={`assertion-item ${c.passed ? 'passed' : 'failed'}`}
                              style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '8px 12px' }}
                            >
                              <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
                                <span style={{ fontSize: '13px', fontWeight: '500', color: 'var(--text-primary)' }}>{c.case_name}</span>
                                <span style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>Status: {c.status} | {c.elapsed_ms} ms</span>
                              </div>
                              {c.passed ? <CheckCircle2 size={14} style={{ color: 'var(--color-get)' }} /> : <AlertCircle size={14} style={{ color: 'var(--color-delete)' }} />}
                            </div>
                          ))}
                        </div>
                      </>
                    ) : (
                      <div className="response-idle-state" style={{ height: '300px' }}>
                        <Terminal size={24} className="idle-icon" />
                        <span className="idle-title" style={{ fontSize: '13px' }}>Suite Runner is ready</span>
                        <p className="idle-text" style={{ fontSize: '12px' }}>Click the Execute Suite button above to run all sweeps in this test suite.</p>
                      </div>
                    )}
                  </div>
                </>
              )}

              {!selectedTestCase && !selectedSuitePath && (
                <div className="response-idle-state">
                  <Play size={28} className="idle-icon" />
                  <h3 className="idle-title">Test Runner is Idle</h3>
                  <p className="idle-text font-normal">Select a suite folder or test case to run assertions sweeps.</p>
                </div>
              )}
            </section>
          </>
        )}

        {/* 3. EXECUTION REPORTS HISTORY VIEWPORTS */}
        {activeView === 'reports' && (
          <>
            {/* Left Sidebar */}
            <section className="sidebar" style={{ width: `${sidebarWidth}px` }}>
              <div className="sidebar-header">
                <span className="sidebar-title">Execution History</span>
              </div>
              <div className="request-tree-container">
                {reports.length === 0 ? (
                  <div style={{ padding: '24px 16px', textAlign: 'center', color: 'var(--text-muted)', fontSize: '13px' }}>
                    No execution reports stored in local database
                  </div>
                ) : (
                  reports.map(rep => {
                    const isActive = selectedReport && selectedReport.id === rep.id;
                    const meta = reportMeta(rep);
                    const methodLabel = meta.method || rep.module;
                    return (
                      <div 
                        key={rep.id} 
                        className={`request-tree-item ${isActive ? 'active' : ''}`}
                        onClick={() => setSelectedReport(rep)}
                        style={{ flexDirection: 'column', alignItems: 'flex-start', gap: '4px', padding: '10px' }}
                      >
                        <div style={{ display: 'flex', width: '100%', justifyContent: 'space-between', alignItems: 'center' }}>
                          <span 
                            className="method-tag" 
                            style={{ 
                              padding: '1px 4px', 
                              fontSize: '8px', 
                              minWidth: 'auto', 
                              backgroundColor: meta.method ? `var(--color-${meta.method.toLowerCase()}, rgba(16, 185, 129, 0.15))` :
                                             rep.module.toLowerCase() === 'security' ? 'rgba(239, 68, 68, 0.15)' : 
                                             rep.module.toLowerCase() === 'performance' ? 'rgba(139, 92, 246, 0.15)' : 
                                             rep.module.toLowerCase() === 'docs' ? 'rgba(245, 158, 11, 0.15)' : 
                                             'rgba(16, 185, 129, 0.15)',
                              color: meta.method ? '#fff' :
                                     rep.module.toLowerCase() === 'security' ? 'var(--color-delete)' : 
                                     rep.module.toLowerCase() === 'performance' ? 'var(--color-patch)' : 
                                     rep.module.toLowerCase() === 'docs' ? 'var(--color-post)' : 
                                     'var(--color-get)'
                            }}
                          >
                            {methodLabel}
                          </span>
                          <span style={{ fontSize: '9px', color: 'var(--text-muted)' }}>{new Date(rep.created_at).toLocaleTimeString()}</span>
                        </div>
                        <span className="request-name" style={{ fontSize: '12px', fontWeight: '500', color: 'var(--text-primary)' }}>{meta.url || rep.name}</span>
                        <div className="history-meta-row">
                          {typeof meta.status === 'number' && (
                            <span className={meta.status < 400 ? 'history-status-ok' : 'history-status-error'}>
                              {meta.status} {meta.reason || ''}
                            </span>
                          )}
                          {typeof meta.elapsed_ms === 'number' && <span>{formatMs(meta.elapsed_ms)}</span>}
                          {typeof meta.size_bytes === 'number' && <span>{formatBytes(meta.size_bytes)}</span>}
                        </div>
                        <span style={{ fontSize: '11px', color: 'var(--text-muted)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', width: '100%' }}>{rep.summary}</span>
                      </div>
                    );
                  })
                )}
              </div>
            </section>

            <div className={`resize-divider ${isResizingSidebar ? 'dragging' : ''}`} onMouseDown={handleMouseDownSidebar} />

            {/* Center Panel: Detailed Report Viewer */}
            <section className="request-pane">
              {selectedReport ? (
                <>
                  <div className="request-header">
                    <div>
                      <span className="select-label" style={{ display: 'block', marginBottom: '4px' }}>Report #{selectedReport.id} ({selectedReport.module})</span>
                      <h2 style={{ fontSize: '18px', fontWeight: '600' }}>{selectedReport.name}</h2>
                      <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Recorded on {new Date(selectedReport.created_at).toLocaleString()}</span>
                    </div>

                    <div style={{ padding: '12px', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '6px', fontSize: '13px', fontWeight: '500' }}>
                      {selectedReport.summary}
                    </div>
                  </div>

                  {(() => {
                    const parsedPayload = parseReportPayload(selectedReport);
                    const selectedMeta = reportMeta(selectedReport);

                    const modLower = selectedReport.module.toLowerCase();
                    if (modLower === 'security') {
                      return (
                        <div className="tab-content" style={{ padding: '20px', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '16px' }}>
                          {parsedPayload && parsedPayload.findings ? (
                            <>
                              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                                <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                                  Source: <strong>{parsedPayload.source}</strong> | Live Scan: <strong>{parsedPayload.live_scan ? "Yes" : "No"}</strong>
                                </span>
                                <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>
                                  Generated: {parsedPayload.generated_at ? new Date(parsedPayload.generated_at).toLocaleString() : ''}
                                </span>
                              </div>

                              <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                                {parsedPayload.findings.length === 0 ? (
                                  <div style={{ padding: '24px', textAlign: 'center', backgroundColor: 'var(--bg-secondary)', borderRadius: '8px', border: '1px solid var(--border-color)', color: 'var(--text-muted)' }}>
                                    No security findings detected for this source.
                                  </div>
                                ) : (
                                  parsedPayload.findings.map((f, idx: number) => {
                                    const severityColors: Record<string, string> = {
                                      critical: 'rgba(239, 68, 68, 0.15)',
                                      high: 'rgba(249, 115, 22, 0.15)',
                                      medium: 'rgba(234, 179, 8, 0.15)',
                                      low: 'rgba(59, 130, 246, 0.15)',
                                    };
                                    const textColors: Record<string, string> = {
                                      critical: 'var(--color-delete)',
                                      high: '#f97316',
                                      medium: 'var(--color-post)',
                                      low: 'var(--color-put)',
                                    };
                                    const severity = f.severity?.toLowerCase() || '';
                                    return (
                                      <div 
                                        key={idx} 
                                        style={{ 
                                          backgroundColor: 'var(--bg-secondary)', 
                                          border: '1px solid var(--border-color)', 
                                          borderRadius: '8px', 
                                          overflow: 'hidden' 
                                        }}
                                      >
                                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 16px', borderBottom: '1px solid var(--border-color)' }}>
                                          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                                            <span 
                                              style={{ 
                                                fontSize: '10px', 
                                                fontWeight: '700', 
                                                textTransform: 'uppercase', 
                                                padding: '2px 6px', 
                                                borderRadius: '4px',
                                                backgroundColor: severityColors[severity] || 'var(--bg-hover)',
                                                color: textColors[severity] || 'var(--text-secondary)'
                                              }}
                                            >
                                              {f.severity}
                                            </span>
                                            <strong style={{ fontSize: '14px', color: 'var(--text-primary)' }}>{f.title}</strong>
                                          </div>
                                          <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Risk Score: <strong style={{ color: textColors[severity] }}>{f.risk_score}</strong></span>
                                        </div>
                                        
                                        <div style={{ padding: '16px', display: 'flex', flexDirection: 'column', gap: '10px', fontSize: '13px', lineHeight: '1.5' }}>
                                          <div>
                                            <span style={{ color: 'var(--text-muted)', display: 'block', fontSize: '11px', textTransform: 'uppercase', fontWeight: '600' }}>Endpoint / Target</span>
                                            <code style={{ color: 'var(--text-secondary)', fontFamily: 'monospace' }}>{f.endpoint}</code>
                                          </div>
                                          <div>
                                            <span style={{ color: 'var(--text-muted)', display: 'block', fontSize: '11px', textTransform: 'uppercase', fontWeight: '600' }}>Impact</span>
                                            <p style={{ color: 'var(--text-secondary)' }}>{f.impact}</p>
                                          </div>
                                          <div>
                                            <span style={{ color: 'var(--text-muted)', display: 'block', fontSize: '11px', textTransform: 'uppercase', fontWeight: '600' }}>Remediation</span>
                                            <p style={{ color: 'var(--text-primary)', fontWeight: '500' }}>{f.remediation}</p>
                                          </div>
                                          {f.evidence && (
                                            <div>
                                              <span style={{ color: 'var(--text-muted)', display: 'block', fontSize: '11px', textTransform: 'uppercase', fontWeight: '600' }}>Evidence</span>
                                              <pre style={{ backgroundColor: 'var(--bg-primary)', padding: '8px 12px', borderRadius: '4px', fontFamily: 'monospace', fontSize: '12px', border: '1px solid var(--border-color)', overflowX: 'auto', whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}><code>{f.evidence}</code></pre>
                                            </div>
                                          )}
                                        </div>
                                      </div>
                                    );
                                  })
                                )}
                              </div>
                            </>
                          ) : (
                            <JsonCode source={selectedReport.payload_json} />
                          )}
                        </div>
                      );
                    }

                    if (modLower === 'performance') {
                      return (
                        <div className="tab-content" style={{ padding: '20px', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '16px' }}>
                          {parsedPayload && parsedPayload.endpoints ? (
                            <>
                              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                                <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>
                                  Source: <strong>{parsedPayload.source}</strong> | Iterations: <strong>{parsedPayload.iterations}</strong>
                                </span>
                                <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>
                                  Generated: {parsedPayload.generated_at ? new Date(parsedPayload.generated_at).toLocaleString() : ''}
                                </span>
                              </div>

                              <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
                                {parsedPayload.endpoints.map((ep, idx: number) => (
                                  <div key={idx} style={{ backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '8px', padding: '16px' }}>
                                    <h4 style={{ fontSize: '14px', fontWeight: '600', marginBottom: '12px', fontFamily: 'monospace', color: 'var(--text-primary)', wordBreak: 'break-all' }}>{ep.endpoint}</h4>
                                    
                                    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(100px, 1fr))', gap: '12px', marginBottom: '16px' }}>
                                      <div style={{ padding: '8px', backgroundColor: 'var(--bg-primary)', borderRadius: '6px', textAlign: 'center' }}>
                                        <span style={{ fontSize: '10px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Samples</span>
                                        <div style={{ fontSize: '15px', fontWeight: 'bold' }}>{ep.samples}</div>
                                      </div>
                                      <div style={{ padding: '8px', backgroundColor: 'var(--bg-primary)', borderRadius: '6px', textAlign: 'center' }}>
                                        <span style={{ fontSize: '10px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Success</span>
                                        <div style={{ fontSize: '15px', fontWeight: 'bold', color: 'var(--color-get)' }}>{ep.success_count}</div>
                                      </div>
                                      <div style={{ padding: '8px', backgroundColor: 'var(--bg-primary)', borderRadius: '6px', textAlign: 'center' }}>
                                        <span style={{ fontSize: '10px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Errors</span>
                                        <div style={{ fontSize: '15px', fontWeight: 'bold', color: (ep.error_count || 0) > 0 ? 'var(--color-delete)' : 'var(--text-muted)' }}>{ep.error_count}</div>
                                      </div>
                                      <div style={{ padding: '8px', backgroundColor: 'var(--bg-primary)', borderRadius: '6px', textAlign: 'center' }}>
                                        <span style={{ fontSize: '10px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Avg Size</span>
                                        <div style={{ fontSize: '15px', fontWeight: 'bold' }}>{ep.avg_size_bytes} B</div>
                                      </div>
                                    </div>

                                    <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                                      <span style={{ fontSize: '10px', textTransform: 'uppercase', color: 'var(--text-muted)', fontWeight: '600' }}>Latency Distribution</span>
                                      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(5, 1fr)', gap: '6px' }}>
                                        <div style={{ padding: '8px', backgroundColor: 'rgba(79, 70, 229, 0.08)', border: '1px solid var(--border-color)', borderRadius: '6px', textAlign: 'center' }}>
                                          <span style={{ fontSize: '9px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Min</span>
                                          <div style={{ fontSize: '13px', fontWeight: '600' }}>{ep.min_ms} ms</div>
                                        </div>
                                        <div style={{ padding: '8px', backgroundColor: 'rgba(79, 70, 229, 0.08)', border: '1px solid var(--border-color)', borderRadius: '6px', textAlign: 'center' }}>
                                          <span style={{ fontSize: '9px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>P50</span>
                                          <div style={{ fontSize: '13px', fontWeight: '600' }}>{ep.p50_ms} ms</div>
                                        </div>
                                        <div style={{ padding: '8px', backgroundColor: 'rgba(79, 70, 229, 0.08)', border: '1px solid var(--border-color)', borderRadius: '6px', textAlign: 'center' }}>
                                          <span style={{ fontSize: '9px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Avg</span>
                                          <div style={{ fontSize: '13px', fontWeight: '600', color: 'var(--accent-hover)' }}>{ep.avg_ms} ms</div>
                                        </div>
                                        <div style={{ padding: '8px', backgroundColor: 'rgba(79, 70, 229, 0.08)', border: '1px solid var(--border-color)', borderRadius: '6px', textAlign: 'center' }}>
                                          <span style={{ fontSize: '9px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>P95</span>
                                          <div style={{ fontSize: '13px', fontWeight: '600' }}>{ep.p95_ms} ms</div>
                                        </div>
                                        <div style={{ padding: '8px', backgroundColor: 'rgba(79, 70, 229, 0.08)', border: '1px solid var(--border-color)', borderRadius: '6px', textAlign: 'center' }}>
                                          <span style={{ fontSize: '9px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Max</span>
                                          <div style={{ fontSize: '13px', fontWeight: '600' }}>{ep.max_ms} ms</div>
                                        </div>
                                      </div>
                                    </div>
                                  </div>
                                ))}
                              </div>
                            </>
                          ) : (
                            <JsonCode source={selectedReport.payload_json} />
                          )}
                        </div>
                      );
                    }

                    if (modLower === 'docs') {
                      return (
                        <div className="tab-content" style={{ padding: '20px', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '16px' }}>
                          {parsedPayload ? (
                            <div style={{ backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '8px', padding: '20px', display: 'flex', flexDirection: 'column', gap: '16px' }}>
                              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                                <h3 style={{ fontSize: '16px', fontWeight: 'bold' }}>API Documentation Spec</h3>
                                <span style={{ fontSize: '10px', fontWeight: '700', textTransform: 'uppercase', padding: '2px 8px', borderRadius: '4px', backgroundColor: 'var(--accent-light)', color: 'var(--accent-hover)' }}>
                                  {parsedPayload.format}
                                </span>
                              </div>

                              <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', fontSize: '13px' }}>
                                <div>
                                  <span style={{ color: 'var(--text-muted)', display: 'block', fontSize: '11px', textTransform: 'uppercase', fontWeight: '600' }}>Source Collection</span>
                                  <strong style={{ color: 'var(--text-primary)' }}>{parsedPayload.source}</strong>
                                </div>
                                <div>
                                  <span style={{ color: 'var(--text-muted)', display: 'block', fontSize: '11px', textTransform: 'uppercase', fontWeight: '600' }}>Endpoints Documented</span>
                                  <strong style={{ color: 'var(--text-primary)' }}>{parsedPayload.request_count} endpoints</strong>
                                </div>
                                <div>
                                  <span style={{ color: 'var(--text-muted)', display: 'block', fontSize: '11px', textTransform: 'uppercase', fontWeight: '600' }}>Generated Output File Path</span>
                                  <code style={{ display: 'block', padding: '10px', backgroundColor: 'var(--bg-primary)', borderRadius: '6px', border: '1px solid var(--border-color)', color: 'var(--text-secondary)', fontFamily: 'monospace', wordBreak: 'break-all' }}>{parsedPayload.output_path}</code>
                                </div>
                                <div>
                                  <span style={{ color: 'var(--text-muted)', display: 'block', fontSize: '11px', textTransform: 'uppercase', fontWeight: '600' }}>Timestamp</span>
                                  <span style={{ color: 'var(--text-secondary)' }}>{parsedPayload.generated_at ? new Date(parsedPayload.generated_at).toLocaleString() : ''}</span>
                                </div>
                              </div>
                            </div>
                          ) : (
                            <JsonCode source={selectedReport.payload_json} />
                          )}
                        </div>
                      );
                    }

                    // Default: HTTP Trace logs visualizer
                    return (
                      <div className="tab-content" style={{ padding: '20px', overflowY: 'auto', display: 'flex', flexDirection: 'column', gap: '16px' }}>
                        {parsedPayload ? (
                          <>
                            <div style={{ display: 'flex', gap: '16px', flexWrap: 'wrap' }}>
                              <div style={{ padding: '12px', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '8px', minWidth: '120px' }}>
                                <div style={{ fontSize: '10px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Method</div>
                                <div style={{ fontSize: '16px', fontWeight: 'bold', color: parsedPayload.method ? `var(--color-${parsedPayload.method.toLowerCase()})` : 'var(--text-primary)' }}>
                                  {parsedPayload.method || 'GET'}
                                </div>
                              </div>
                              <div style={{ padding: '12px', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '8px', minWidth: '120px' }}>
                                <div style={{ fontSize: '10px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Status</div>
                                <div style={{ fontSize: '16px', fontWeight: 'bold', color: (parsedPayload.status || 0) < 400 ? 'var(--color-get)' : 'var(--color-delete)' }}>
                                  {parsedPayload.status} {parsedPayload.reason}
                                </div>
                              </div>
                              <div style={{ padding: '12px', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '8px', minWidth: '120px' }}>
                                <div style={{ fontSize: '10px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Duration</div>
                                <div style={{ fontSize: '16px', fontWeight: 'bold', color: 'var(--color-put)' }}>{parsedPayload.elapsed_ms} ms</div>
                              </div>
                              <div style={{ padding: '12px', backgroundColor: 'var(--bg-secondary)', border: '1px solid var(--border-color)', borderRadius: '8px', minWidth: '120px' }}>
                                <div style={{ fontSize: '10px', color: 'var(--text-muted)', textTransform: 'uppercase' }}>Size</div>
                                <div style={{ fontSize: '16px', fontWeight: 'bold' }}>{parsedPayload.size_bytes} B</div>
                              </div>
                            </div>

                            <div className="history-detail-grid">
                              <div>
                                <span>Request URL</span>
                                <code>{selectedMeta.url || selectedReport.name}</code>
                              </div>
                              <div>
                                <span>Final URL</span>
                                <code>{selectedMeta.final_url || selectedMeta.url || selectedReport.name}</code>
                              </div>
                              <div>
                                <span>Content Type</span>
                                <code>{selectedMeta.content_type || 'unknown'}</code>
                              </div>
                            </div>

                            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                              <span style={{ fontSize: '11px', textTransform: 'uppercase', color: 'var(--text-muted)', fontWeight: '600' }}>Response Body</span>
                              <div className="response-body-wrapper" style={{ borderRadius: '8px', border: '1px solid var(--border-color)' }}>
                                {isJsonText(parsedPayload.body) ? (
                                  <JsonCode source={parsedPayload.body || ''} />
                                ) : (
                                  <pre className="response-body-pre"><code>{parsedPayload.body}</code></pre>
                                )}
                              </div>
                            </div>
                          </>
                        ) : (
                          <JsonCode source={selectedReport.payload_json} />
                        )}
                      </div>
                    );
                  })()}
                </>
              ) : (
                <div className="response-idle-state" style={{ backgroundColor: 'var(--bg-primary)' }}>
                  <div style={{ padding: '24px', backgroundColor: 'var(--bg-secondary)', borderRadius: '50%', border: '1px solid var(--border-color)' }}>
                    <Database size={32} className="idle-icon" />
                  </div>
                  <h3 className="idle-title">Select a history report</h3>
                  <p className="idle-text font-normal">Select an execution log record from the database list on the left to inspect detailed traces.</p>
                </div>
              )}
            </section>

            <div className={`resize-divider ${isResizingResponse ? 'dragging' : ''}`} onMouseDown={handleMouseDownResponse} />

            {/* Right Panel: Empty/Summary panel for reports */}
            <section className="response-pane" style={{ width: `${responseWidth}px` }}>
              <div className="response-idle-state">
                <Terminal size={28} className="idle-icon" />
                <h3 className="idle-title">System Audit Log</h3>
                <p className="idle-text font-normal">Reports are automatically populated inside the SQLite database whenever requests are run through CLI, TUI, or GUI modules.</p>
              </div>
            </section>
          </>
        )}
      </main>

      {/* Workspace Creation Modal */}
      {showCreateWorkspace && (
        <div className="dialog-overlay">
          <div className="dialog-content">
            <h3 className="dialog-title">Create Workspace</h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Name</label>
                <input 
                  type="text" 
                  className="kv-input" 
                  value={newWsName} 
                  onChange={(e) => setNewWsName(e.target.value)}
                  placeholder="e.g. Project Alpha"
                />
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Description</label>
                <input 
                  type="text" 
                  className="kv-input" 
                  value={newWsDesc} 
                  onChange={(e) => setNewWsDesc(e.target.value)}
                  placeholder="API workbench for Project Alpha"
                />
              </div>
            </div>
            <div className="dialog-buttons">
              <button className="btn btn-secondary" onClick={() => setShowCreateWorkspace(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handleCreateWorkspace}>Create</button>
            </div>
          </div>
        </div>
      )}

      {/* Workspace Renaming Modal */}
      {showRenameWorkspace && (
        <div className="dialog-overlay">
          <div className="dialog-content">
            <h3 className="dialog-title">Rename Workspace</h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>New Name</label>
                <input 
                  type="text" 
                  className="kv-input" 
                  value={newWsRenameName} 
                  onChange={(e) => setNewWsRenameName(e.target.value)}
                  placeholder="e.g. Project Beta"
                />
              </div>
            </div>
            <div className="dialog-buttons">
              <button className="btn btn-secondary" onClick={() => setShowRenameWorkspace(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handleRenameWorkspace}>Rename</button>
            </div>
          </div>
        </div>
      )}

      {/* Settings Info Modal */}
      {showSettings && (
        <div className="dialog-overlay">
          <div className="dialog-content" style={{ width: '480px' }}>
            <h3 className="dialog-title" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <SlidersHorizontal size={18} />
              <span>ZapReq Settings</span>
            </h3>

            <div className="tabs-row" style={{ padding: 0, marginBottom: '12px', backgroundColor: 'transparent' }}>
              <button 
                className={`tab-btn ${settingsTab === 'general' ? 'active' : ''}`} 
                onClick={() => setSettingsTab('general')}
                style={{ padding: '8px 12px' }}
              >
                General Info
              </button>
              <button 
                className={`tab-btn ${settingsTab === 'secrets' ? 'active' : ''}`} 
                onClick={() => setSettingsTab('secrets')}
                style={{ padding: '8px 12px' }}
              >
                Secrets Manager
              </button>
            </div>
            
            {settingsTab === 'general' && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '14px', fontSize: '13px', color: 'var(--text-secondary)', lineHeight: '1.6' }}>
                <div style={{ display: 'flex', gap: '10px', alignItems: 'center', padding: '10px', backgroundColor: 'var(--bg-primary)', borderRadius: '6px', border: '1px solid var(--border-color)' }}>
                  <ShieldCheck size={20} style={{ color: 'var(--color-get)' }} />
                  <span>Rust backend engine connected successfully.</span>
                </div>

                <p>
                  ZapReq has been migrated to a high-performance Tauri + React architecture. All key operations (HTTP request execution, collection serialization, sqlite database operations) are delegated directly to the local Rust core.
                </p>

                <div>
                  <h4 style={{ color: 'var(--text-primary)', fontWeight: '600', marginBottom: '4px' }}>Theme and Layout</h4>
                  <p>Features custom resizable sidebars and panels. Pane widths are saved locally using custom Tauri settings and will be remembered on subsequent launches.</p>
                </div>

                <div style={{ borderTop: '1px solid var(--border-color)', paddingTop: '12px', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                    <Heart size={14} style={{ color: 'red' }} fill="red" />
                    <span>Version 0.1.4 (Tauri 2.0)</span>
                  </div>
                  <span>ZapReq Core Engine</span>
                </div>
              </div>
            )}

            {settingsTab === 'secrets' && (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '14px', fontSize: '13px' }}>
                <p style={{ color: 'var(--text-secondary)', lineHeight: '1.5' }}>
                  Manage native secrets. Reference key names using double curly braces, e.g. <code>{"{{MY_API_KEY}}"}</code>. Values are loaded securely at runtime and never saved in workspaces.
                </p>
                
                {/* Add new secret form */}
                <div style={{ display: 'flex', gap: '8px', alignItems: 'flex-end', backgroundColor: 'var(--bg-primary)', padding: '10px', borderRadius: '6px', border: '1px solid var(--border-color)' }}>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', flex: 1 }}>
                    <label style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Key Name</label>
                    <input 
                      type="text" 
                      placeholder="e.g. AWS_SECRET" 
                      className="kv-input" 
                      value={newSecretKey} 
                      onChange={(e) => setNewSecretKey(e.target.value.toUpperCase().replace(/[^A-Z0-9_]/g, ''))}
                      style={{ padding: '6px 10px' }}
                    />
                  </div>
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', flex: 1.5 }}>
                    <label style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Secret Value</label>
                    <input 
                      type="password" 
                      placeholder="value" 
                      className="kv-input" 
                      value={newSecretVal} 
                      onChange={(e) => setNewSecretVal(e.target.value)}
                      style={{ padding: '6px 10px' }}
                    />
                  </div>
                  <button className="btn btn-primary" onClick={handleAddSecret} style={{ padding: '6px 12px', height: '32px' }}>
                    Add
                  </button>
                </div>

                {/* Secrets list */}
                <div style={{ maxHeight: '180px', overflowY: 'auto', border: '1px solid var(--border-color)', borderRadius: '6px' }}>
                  {secrets.length === 0 ? (
                    <div style={{ padding: '16px', color: 'var(--text-muted)', textAlign: 'center' }}>
                      No active secrets defined.
                    </div>
                  ) : (
                    secrets.map(key => (
                      <div key={key} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: '8px 12px', borderBottom: '1px solid var(--border-color)', backgroundColor: 'var(--bg-primary)' }}>
                        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                          <Key size={14} style={{ color: 'var(--accent-hover)' }} />
                          <span style={{ fontFamily: 'monospace', fontWeight: '600' }}>{key}</span>
                        </div>
                        <button className="icon-btn" onClick={() => handleDeleteSecret(key)} title="Delete Secret">
                          <Trash size={14} style={{ color: 'var(--color-delete)' }} />
                        </button>
                      </div>
                    ))
                  )}
                </div>
              </div>
            )}

            <div className="dialog-buttons">
              <button className="btn btn-primary" onClick={() => setShowSettings(false)}>Close</button>
            </div>
          </div>
        </div>
      )}

      {/* Import Workspace Modal */}
      {showImportDialog && (
        <div className="dialog-overlay">
          <div className="dialog-content">
            <h3 className="dialog-title">Import Collection / Workspace</h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>New Workspace Name</label>
                <input 
                  type="text" 
                  className="kv-input" 
                  value={importWsName} 
                  onChange={(e) => setImportWsName(e.target.value)}
                  placeholder="e.g. My Imported API"
                />
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Import JSON File</label>
                <input 
                  type="text" 
                  className="kv-input" 
                  value={importFilePath} 
                  onChange={(e) => setImportFilePath(e.target.value)}
                  placeholder="e.g. /absolute/path/to/collection.json"
                />
                <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Supported: ZapReq workspace JSON, Postman collection JSON, OpenAPI JSON, or legacy ZapReq request JSON.</span>
              </div>
            </div>
            <div className="dialog-buttons">
              <button className="btn btn-secondary" onClick={() => setShowImportDialog(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handleImportWorkspace}>Import</button>
            </div>
          </div>
        </div>
      )}

      {/* Export Workspace Modal */}
      {showExportDialog && (
        <div className="dialog-overlay">
          <div className="dialog-content">
            <h3 className="dialog-title">Export Workspace: {activeWorkspaceName}</h3>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Format</label>
                <select 
                  className="toolbar-select" 
                  value={exportFormat} 
                  onChange={(e) => {
                    const nextFormat = e.target.value;
                    const currentDefault = defaultExportFilename(activeWorkspaceName, exportFormat);
                    setExportFormat(nextFormat);
                    setExportFilePath(prev => !prev || prev === currentDefault ? defaultExportFilename(activeWorkspaceName, nextFormat) : prev);
                  }}
                  style={{ width: '100%' }}
                >
                  <option value="zapreq">ZapReq Workspace JSON</option>
                  <option value="postman">Postman Collection v2.1</option>
                  <option value="openapi">OpenAPI Spec 3.0 (JSON)</option>
                </select>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                <label style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Destination File or Folder</label>
                <input 
                  type="text" 
                  className="kv-input" 
                  value={exportFilePath} 
                  onChange={(e) => setExportFilePath(e.target.value)}
                  placeholder={`e.g. /absolute/path/to/${defaultExportFilename(activeWorkspaceName, exportFormat)}`}
                />
                <span style={{ fontSize: '11px', color: 'var(--text-muted)' }}>Enter a full file path, or enter an existing folder and ZapReq will create {defaultExportFilename(activeWorkspaceName, exportFormat)} inside it.</span>
              </div>
            </div>
            <div className="dialog-buttons">
              <button className="btn btn-secondary" onClick={() => setShowExportDialog(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handleExportWorkspace}>Export</button>
            </div>
          </div>
        </div>
      )}

      {/* Create Test Case Modal */}
      {showAddTestDialog && (
        <div className="dialog-overlay">
          <div className="dialog-content" style={{ width: '420px' }}>
            <h3 className="dialog-title" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <ShieldCheck size={18} style={{ color: 'var(--accent-hover)' }} />
              <span>Create Regression Test Case</span>
            </h3>
            
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <label style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Suite Name</label>
                <input 
                  type="text" 
                  className="kv-input" 
                  value={testSuiteName} 
                  onChange={(e) => setTestSuiteName(e.target.value)}
                  placeholder="e.g. Auth Sweep"
                />
              </div>

              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <label style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Source Request</label>
                <select
                  className="kv-input"
                  value={selectedRequestForTestCaseId}
                  onChange={(e) => {
                    const val = e.target.value;
                    setSelectedRequestForTestCaseId(val);
                    if (val && val !== "custom") {
                      const req = currentWorkspace?.requests.find(r => r.id === val);
                      if (req) {
                        setTestCaseName(req.name);
                      }
                    }
                  }}
                  style={{ width: '100%' }}
                >
                  {currentWorkspace?.requests.map(r => (
                    <option key={r.id} value={r.id || ""}>{r.name} ({r.method})</option>
                  ))}
                  <option value="custom">Custom Request...</option>
                </select>
              </div>

              {selectedRequestForTestCaseId === "custom" && (
                <>
                  <div style={{ display: 'flex', gap: '8px', alignItems: 'flex-end' }}>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', width: '90px' }}>
                      <label style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Method</label>
                      <select 
                        className="kv-input" 
                        value={tcRequestMethod} 
                        onChange={(e) => setTcRequestMethod(e.target.value)}
                        style={{ minWidth: 'auto', width: '100%', cursor: 'pointer' }}
                      >
                        <option value="GET">GET</option>
                        <option value="POST">POST</option>
                        <option value="PUT">PUT</option>
                        <option value="DELETE">DELETE</option>
                        <option value="PATCH">PATCH</option>
                      </select>
                    </div>
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', flex: 1 }}>
                      <label style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>URL</label>
                      <input 
                        type="text" 
                        className="kv-input" 
                        value={tcRequestUrl} 
                        onChange={(e) => setTcRequestUrl(e.target.value)}
                        placeholder="https://api.example.com/endpoint"
                      />
                    </div>
                  </div>
                </>
              )}
              
              <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                <label style={{ fontSize: '12px', color: 'var(--text-secondary)' }}>Test Case Name</label>
                <input 
                  type="text" 
                  className="kv-input" 
                  value={testCaseName} 
                  onChange={(e) => setTestCaseName(e.target.value)}
                  placeholder="e.g. Validate Valid Token"
                />
              </div>
              
              <div style={{ borderTop: '1px solid var(--border-color)', margin: '8px 0', paddingTop: '8px' }}>
                <span style={{ fontSize: '11px', textTransform: 'uppercase', color: 'var(--text-muted)', fontWeight: '600', display: 'block', marginBottom: '8px' }}>Expectations (Assertions)</span>
                
                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' }}>
                    <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Expected Status Code</span>
                    <input 
                      type="text" 
                      className="kv-input" 
                      value={expectStatus} 
                      onChange={(e) => setExpectStatus(e.target.value.replace(/[^0-9]/g, ''))}
                      placeholder="e.g. 200"
                      style={{ width: '80px', textAlign: 'center', padding: '4px' }}
                    />
                  </div>

                  <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '8px' }}>
                    <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Max Response Time (ms)</span>
                    <input 
                      type="text" 
                      className="kv-input" 
                      value={expectMaxTime} 
                      onChange={(e) => setExpectMaxTime(e.target.value.replace(/[^0-9]/g, ''))}
                      placeholder="e.g. 500"
                      style={{ width: '80px', textAlign: 'center', padding: '4px' }}
                    />
                  </div>

                  <div style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
                    <span style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Response Contains Substring</span>
                    <input 
                      type="text" 
                      className="kv-input" 
                      value={expectContains} 
                      onChange={(e) => setExpectContains(e.target.value)}
                      placeholder="e.g. access_token"
                    />
                  </div>
                </div>
              </div>
            </div>
            
            <div className="dialog-buttons" style={{ marginTop: '8px' }}>
              <button className="btn btn-secondary" onClick={() => setShowAddTestDialog(false)}>Cancel</button>
              <button className="btn btn-primary" onClick={handleSaveTestCase}>Create</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
