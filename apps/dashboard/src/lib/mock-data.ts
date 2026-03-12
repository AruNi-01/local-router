import type { Project, Workspace, ServiceDef, Instance, Route, LogEntry, GraphNode, GraphEdge } from './types';

export const mockProjects: Project[] = [
  {
    id: 'proj-1',
    name: 'atmos',
    path: '~/code/atmos',
    workspaces: [],
    services: [],
    createdAt: '2026-03-01',
  },
  {
    id: 'proj-2',
    name: 'localrouter',
    path: '~/code/localrouter',
    workspaces: [],
    services: [],
    createdAt: '2026-02-15',
  },
  {
    id: 'proj-3',
    name: 'vibe-trader',
    path: '~/code/vibe-trader',
    workspaces: [],
    services: [],
    createdAt: '2026-03-05',
  },
];

export const mockWorkspaces: Workspace[] = [
  { id: 'ws-1', projectId: 'proj-1', name: 'main', branch: 'main', path: '~/code/atmos', isActive: true, instances: [] },
  { id: 'ws-2', projectId: 'proj-1', name: 'feat-auth', branch: 'feat/auth', path: '~/code/atmos-feat-auth', isActive: true, instances: [] },
  { id: 'ws-3', projectId: 'proj-2', name: 'main', branch: 'main', path: '~/code/localrouter', isActive: true, instances: [] },
  { id: 'ws-4', projectId: 'proj-3', name: 'main', branch: 'main', path: '~/code/vibe-trader', isActive: true, instances: [] },
  { id: 'ws-5', projectId: 'proj-3', name: 'feat-charts', branch: 'feat/charts', path: '~/code/vibe-trader-charts', isActive: false, instances: [] },
];

export const mockServices: ServiceDef[] = [
  { id: 'svc-1', projectId: 'proj-1', name: 'web', command: 'next dev --port ${PORT}', protocol: 'http', adapter: 'nextjs', route: 'web', healthcheck: 'http://127.0.0.1:${PORT}', language: 'typescript' },
  { id: 'svc-2', projectId: 'proj-1', name: 'api', command: 'cargo run --bin api -- --port ${PORT}', protocol: 'http', adapter: 'rust', route: 'api', healthcheck: 'http://127.0.0.1:${PORT}/healthz', language: 'rust' },
  { id: 'svc-3', projectId: 'proj-1', name: 'worker', command: 'python worker.py', protocol: 'none', adapter: 'python', route: 'none', healthcheck: '', language: 'python' },
  { id: 'svc-4', projectId: 'proj-2', name: 'daemon', command: './target/release/localrouter-daemon', protocol: 'http', adapter: 'rust', route: 'daemon', healthcheck: 'http://127.0.0.1:${PORT}/health', language: 'rust' },
  { id: 'svc-5', projectId: 'proj-2', name: 'docs', command: 'vitepress dev --port ${PORT}', protocol: 'http', adapter: 'vite', route: 'docs', healthcheck: 'http://127.0.0.1:${PORT}', language: 'typescript' },
  { id: 'svc-6', projectId: 'proj-3', name: 'frontend', command: 'vite dev --port ${PORT}', protocol: 'http', adapter: 'vite', route: 'web', healthcheck: 'http://127.0.0.1:${PORT}', language: 'typescript' },
  { id: 'svc-7', projectId: 'proj-3', name: 'api', command: 'uvicorn main:app --port ${PORT}', protocol: 'http', adapter: 'uvicorn', route: 'api', healthcheck: 'http://127.0.0.1:${PORT}/health', language: 'python' },
];

export const mockInstances: Instance[] = [
  { id: 'inst-1', serviceId: 'svc-1', serviceName: 'web', workspaceId: 'ws-1', workspaceName: 'main', projectId: 'proj-1', projectName: 'atmos', port: 3001, pid: 42310, status: 'healthy', url: 'main.web.atmos.localhost', uptime: '2h 14m', cpu: 3.2, memory: 184 },
  { id: 'inst-2', serviceId: 'svc-2', serviceName: 'api', workspaceId: 'ws-1', workspaceName: 'main', projectId: 'proj-1', projectName: 'atmos', port: 3002, pid: 42311, status: 'healthy', url: 'main.api.atmos.localhost', uptime: '2h 14m', cpu: 1.8, memory: 56 },
  { id: 'inst-3', serviceId: 'svc-3', serviceName: 'worker', workspaceId: 'ws-1', workspaceName: 'main', projectId: 'proj-1', projectName: 'atmos', port: 0, pid: 42312, status: 'healthy', url: '', uptime: '2h 13m', cpu: 0.5, memory: 42 },
  { id: 'inst-4', serviceId: 'svc-1', serviceName: 'web', workspaceId: 'ws-2', workspaceName: 'feat-auth', projectId: 'proj-1', projectName: 'atmos', port: 3003, pid: 42400, status: 'healthy', url: 'feat-auth.web.atmos.localhost', uptime: '45m', cpu: 2.9, memory: 178 },
  { id: 'inst-5', serviceId: 'svc-2', serviceName: 'api', workspaceId: 'ws-2', workspaceName: 'feat-auth', projectId: 'proj-1', projectName: 'atmos', port: 3004, pid: 42401, status: 'unhealthy', url: 'feat-auth.api.atmos.localhost', uptime: '45m', cpu: 12.1, memory: 312 },
  { id: 'inst-6', serviceId: 'svc-4', serviceName: 'daemon', workspaceId: 'ws-3', workspaceName: 'main', projectId: 'proj-2', projectName: 'localrouter', port: 9800, pid: 41000, status: 'healthy', url: 'main.daemon.localrouter.localhost', uptime: '5h 30m', cpu: 0.3, memory: 28 },
  { id: 'inst-7', serviceId: 'svc-5', serviceName: 'docs', workspaceId: 'ws-3', workspaceName: 'main', projectId: 'proj-2', projectName: 'localrouter', port: 9801, pid: 41001, status: 'starting', url: 'main.docs.localrouter.localhost', uptime: '12s', cpu: 8.0, memory: 95 },
  { id: 'inst-8', serviceId: 'svc-6', serviceName: 'frontend', workspaceId: 'ws-4', workspaceName: 'main', projectId: 'proj-3', projectName: 'vibe-trader', port: 5173, pid: 50100, status: 'healthy', url: 'main.web.vibe-trader.localhost', uptime: '1h 02m', cpu: 2.1, memory: 156 },
  { id: 'inst-9', serviceId: 'svc-7', serviceName: 'api', workspaceId: 'ws-4', workspaceName: 'main', projectId: 'proj-3', projectName: 'vibe-trader', port: 8000, pid: 50101, status: 'stopped', url: 'main.api.vibe-trader.localhost', uptime: '—', cpu: 0, memory: 0 },
];

export const mockRoutes: Route[] = [
  { id: 'rt-1', pattern: 'main.web.atmos.localhost', target: '127.0.0.1:3001', serviceId: 'svc-1', serviceName: 'web', workspaceId: 'ws-1', workspaceName: 'main', projectId: 'proj-1', projectName: 'atmos', status: 'active' },
  { id: 'rt-2', pattern: 'main.api.atmos.localhost', target: '127.0.0.1:3002', serviceId: 'svc-2', serviceName: 'api', workspaceId: 'ws-1', workspaceName: 'main', projectId: 'proj-1', projectName: 'atmos', status: 'active' },
  { id: 'rt-3', pattern: 'feat-auth.web.atmos.localhost', target: '127.0.0.1:3003', serviceId: 'svc-1', serviceName: 'web', workspaceId: 'ws-2', workspaceName: 'feat-auth', projectId: 'proj-1', projectName: 'atmos', status: 'active' },
  { id: 'rt-4', pattern: 'feat-auth.api.atmos.localhost', target: '127.0.0.1:3004', serviceId: 'svc-2', serviceName: 'api', workspaceId: 'ws-2', workspaceName: 'feat-auth', projectId: 'proj-1', projectName: 'atmos', status: 'stale' },
  { id: 'rt-5', pattern: 'main.daemon.localrouter.localhost', target: '127.0.0.1:9800', serviceId: 'svc-4', serviceName: 'daemon', workspaceId: 'ws-3', workspaceName: 'main', projectId: 'proj-2', projectName: 'localrouter', status: 'active' },
  { id: 'rt-6', pattern: 'main.docs.localrouter.localhost', target: '127.0.0.1:9801', serviceId: 'svc-5', serviceName: 'docs', workspaceId: 'ws-3', workspaceName: 'main', projectId: 'proj-2', projectName: 'localrouter', status: 'active' },
  { id: 'rt-7', pattern: 'main.web.vibe-trader.localhost', target: '127.0.0.1:5173', serviceId: 'svc-6', serviceName: 'frontend', workspaceId: 'ws-4', workspaceName: 'main', projectId: 'proj-3', projectName: 'vibe-trader', status: 'active' },
  { id: 'rt-8', pattern: 'main.api.vibe-trader.localhost', target: '127.0.0.1:8000', serviceId: 'svc-7', serviceName: 'api', workspaceId: 'ws-4', workspaceName: 'main', projectId: 'proj-3', projectName: 'vibe-trader', status: 'conflict' },
];

export const mockLogs: LogEntry[] = [
  { timestamp: '2026-03-08T10:14:32Z', level: 'info', source: 'atmos/web', message: 'Ready on http://localhost:3001' },
  { timestamp: '2026-03-08T10:14:31Z', level: 'info', source: 'atmos/api', message: 'Listening on 0.0.0.0:3002' },
  { timestamp: '2026-03-08T10:14:30Z', level: 'warn', source: 'atmos/api[feat-auth]', message: 'High memory usage detected: 312MB' },
  { timestamp: '2026-03-08T10:14:29Z', level: 'error', source: 'atmos/api[feat-auth]', message: 'Health check failed: connection refused on /healthz' },
  { timestamp: '2026-03-08T10:14:28Z', level: 'info', source: 'localrouter/daemon', message: 'Daemon started on pid 41000' },
  { timestamp: '2026-03-08T10:14:27Z', level: 'debug', source: 'localrouter/docs', message: 'VitePress dev server starting...' },
  { timestamp: '2026-03-08T10:14:26Z', level: 'info', source: 'vibe-trader/frontend', message: 'Vite dev server running at http://localhost:5173' },
  { timestamp: '2026-03-08T10:14:25Z', level: 'error', source: 'vibe-trader/api', message: 'Process exited with code 1' },
  { timestamp: '2026-03-08T10:14:24Z', level: 'info', source: 'proxy', message: 'Route registered: main.web.atmos.localhost -> 127.0.0.1:3001' },
  { timestamp: '2026-03-08T10:14:23Z', level: 'warn', source: 'proxy', message: 'Route conflict detected: main.api.vibe-trader.localhost' },
];

export const mockGraphNodes: GraphNode[] = [
  { id: 'proj-1', type: 'project', label: 'atmos', status: 'healthy' },
  { id: 'ws-1', type: 'workspace', label: 'main', status: 'healthy' },
  { id: 'ws-2', type: 'workspace', label: 'feat-auth', status: 'unhealthy' },
  { id: 'svc-1-ws1', type: 'service', label: 'web', status: 'healthy' },
  { id: 'svc-2-ws1', type: 'service', label: 'api', status: 'healthy' },
  { id: 'svc-3-ws1', type: 'service', label: 'worker', status: 'healthy' },
  { id: 'svc-1-ws2', type: 'service', label: 'web', status: 'healthy' },
  { id: 'svc-2-ws2', type: 'service', label: 'api', status: 'unhealthy' },
  { id: 'rt-1', type: 'route', label: 'main.web.atmos.localhost' },
  { id: 'rt-2', type: 'route', label: 'main.api.atmos.localhost' },
  { id: 'rt-3', type: 'route', label: 'feat-auth.web.atmos.localhost' },
  { id: 'rt-4', type: 'route', label: 'feat-auth.api.atmos.localhost' },
];

export const mockGraphEdges: GraphEdge[] = [
  { source: 'proj-1', target: 'ws-1', type: 'contains' },
  { source: 'proj-1', target: 'ws-2', type: 'contains' },
  { source: 'ws-1', target: 'svc-1-ws1', type: 'contains' },
  { source: 'ws-1', target: 'svc-2-ws1', type: 'contains' },
  { source: 'ws-1', target: 'svc-3-ws1', type: 'contains' },
  { source: 'ws-2', target: 'svc-1-ws2', type: 'contains' },
  { source: 'ws-2', target: 'svc-2-ws2', type: 'contains' },
  { source: 'svc-1-ws1', target: 'rt-1', type: 'exposes' },
  { source: 'svc-2-ws1', target: 'rt-2', type: 'exposes' },
  { source: 'svc-1-ws2', target: 'rt-3', type: 'exposes' },
  { source: 'svc-2-ws2', target: 'rt-4', type: 'exposes' },
  { source: 'svc-1-ws1', target: 'svc-2-ws1', type: 'depends_on' },
  { source: 'svc-1-ws2', target: 'svc-2-ws2', type: 'depends_on' },
];

// Helper functions
export function getProjectInstances(projectId: string) {
  return mockInstances.filter(i => i.projectId === projectId);
}
export function getWorkspaceInstances(workspaceId: string) {
  return mockInstances.filter(i => i.workspaceId === workspaceId);
}
export function getProjectRoutes(projectId: string) {
  return mockRoutes.filter(r => r.projectId === projectId);
}
export function getProjectWorkspaces(projectId: string) {
  return mockWorkspaces.filter(w => w.projectId === projectId);
}
export function getProjectServices(projectId: string) {
  return mockServices.filter(s => s.projectId === projectId);
}

export const stats = {
  totalProjects: mockProjects.length,
  activeWorkspaces: mockWorkspaces.filter(w => w.isActive).length,
  runningInstances: mockInstances.filter(i => i.status === 'healthy' || i.status === 'starting').length,
  unhealthyInstances: mockInstances.filter(i => i.status === 'unhealthy').length,
  stoppedInstances: mockInstances.filter(i => i.status === 'stopped').length,
  activeRoutes: mockRoutes.filter(r => r.status === 'active').length,
  conflictRoutes: mockRoutes.filter(r => r.status === 'conflict').length,
};
