export type HealthStatus = 'healthy' | 'unhealthy' | 'starting' | 'stopped' | 'unknown';

export interface Project {
  id: string;
  name: string;
  path: string;
  createdAt: string;
  configSource?: string;
  proxyDisabled?: boolean;
}

export interface Workspace {
  id: string;
  projectId: string;
  name: string;
  branch: string;
  path: string;
  isActive: boolean;
  slug?: string;
}

export interface ServiceDef {
  id: string;
  projectId: string;
  name: string;
  command: string;
  protocol: string;
  adapter: string;
  route: string;
  healthcheck: string;
  language: string;
  cwd?: string | null;
  env: Record<string, string>;
  dependsOn: string[];
  enabled: boolean;
}

export interface Instance {
  id: string;
  serviceId: string;
  serviceName: string;
  workspaceId: string;
  workspaceName: string;
  projectId: string;
  projectName: string;
  port: number;
  pid: number;
  status: HealthStatus;
  url: string;
  uptime: string;
  cpu: number;
  memory: number;
  startedAt?: string | null;
  lastExit?: number | null;
  statusReason?: string | null;
}

export interface Route {
  id: string;
  pattern: string;
  url: string;
  target: string;
  serviceId: string;
  serviceName: string;
  workspaceId: string;
  workspaceName: string;
  projectId: string;
  projectName: string;
  status: 'active' | 'stale' | 'conflict';
  conflictReason?: string | null;
}

export interface LogEntry {
  timestamp: string;
  level: 'info' | 'warn' | 'error' | 'debug';
  source: string;
  message: string;
}

export interface GraphNode {
  id: string;
  type: 'project' | 'workspace' | 'service' | 'route';
  label: string;
  status?: HealthStatus;
}

export interface GraphEdge {
  source: string;
  target: string;
  type: 'contains' | 'depends_on' | 'exposes' | 'proxies_to';
}

export interface ProjectDetail {
  project: Project;
  workspaces: Workspace[];
  services: ServiceDef[];
  instances: Instance[];
  routes: Route[];
  manifest: string;
}

export interface GraphSnapshot {
  nodes: GraphNode[];
  edges: GraphEdge[];
  generatedAt: string;
}

export interface DashboardStats {
  totalProjects: number;
  activeWorkspaces: number;
  runningInstances: number;
  unhealthyInstances: number;
  stoppedInstances: number;
  activeRoutes: number;
  conflictRoutes: number;
}

export interface HealthResponse {
  ok: boolean;
  daemon: string;
  apiPort: number;
  proxyPort: number;
  counts: DashboardStats;
}

export interface DaemonConfig {
  apiPort: number;
  proxyPort: number;
  dnsSuffix: string;
  logLevel: string;
  healthcheckInterval: number;
  autoDetect: boolean;
  hotReload: boolean;
}

export interface ManifestUpdateRequest {
  manifest: string;
}
