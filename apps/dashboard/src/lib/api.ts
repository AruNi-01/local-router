import type {
  DaemonConfig,
  GraphSnapshot,
  HealthResponse,
  Instance,
  LogEntry,
  ManifestUpdateRequest,
  Project,
  ProjectDetail,
  Route,
  ServiceDef,
  Workspace,
} from './types';

const API_BASE = import.meta.env.VITE_LOCALROUTER_API ?? 'http://127.0.0.1:9731/v1';

function apiUrl(path: string) {
  return `${API_BASE}${path}`;
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(apiUrl(path), {
    headers: { 'Content-Type': 'application/json', ...(init?.headers ?? {}) },
    ...init,
  });
  if (!response.ok) {
    const message = await response.text();
    throw new Error(message || `Request failed: ${response.status}`);
  }
  if (response.status === 204) {
    return undefined as T;
  }
  return response.json() as Promise<T>;
}

export const api = {
  apiUrl,
  health: () => request<HealthResponse>('/health'),
  config: () => request<DaemonConfig>('/config'),
  saveConfig: (config: DaemonConfig) =>
    request<DaemonConfig>('/config', { method: 'PUT', body: JSON.stringify(config) }),
  projects: () => request<Project[]>('/projects'),
  addProject: (path: string) =>
    request<ProjectDetail>('/projects', { method: 'POST', body: JSON.stringify({ path }) }),
  deleteProject: (projectId: string) =>
    request<void>(`/projects/${projectId}`, { method: 'DELETE' }),
  project: (projectId: string) => request<ProjectDetail>(`/projects/${projectId}`),
  rescanProject: (projectId: string) => request<ProjectDetail>(`/projects/${projectId}/rescan`, { method: 'POST' }),
  saveManifest: (projectId: string, payload: ManifestUpdateRequest) =>
    request<ProjectDetail>(`/projects/${projectId}/manifest`, {
      method: 'PUT',
      body: JSON.stringify(payload),
    }),
  workspaces: () => request<Workspace[]>('/workspaces'),
  services: () => request<ServiceDef[]>('/services'),
  instances: () => request<Instance[]>('/instances'),
  startInstance: (instanceId: string) => request<Instance>(`/instances/${instanceId}/start`, { method: 'POST' }),
  stopInstance: (instanceId: string) => request<Instance>(`/instances/${instanceId}/stop`, { method: 'POST' }),
  restartInstance: (instanceId: string) => request<Instance>(`/instances/${instanceId}/restart`, { method: 'POST' }),
  routes: () => request<Route[]>('/routes'),
  logs: (instanceId?: string) =>
    request<LogEntry[]>(instanceId ? `/logs?instance_id=${instanceId}` : '/logs'),
  graph: () => request<GraphSnapshot>('/graph'),
};
