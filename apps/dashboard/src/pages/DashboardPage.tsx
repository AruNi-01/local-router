import { Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import { useQuery } from '@tanstack/react-query';
import { api } from '@/lib/api';
import { StatusDot, StatusBadge, RouteStatusBadge } from '@/components/StatusIndicators';
import { InstanceActions } from '@/components/InstanceActions';
import { useInstanceControl } from '@/hooks/use-instance-control';
import { DashboardLayout } from '@/components/DashboardLayout';
import {
  FolderKanban,
  GitBranch,
  Play,
  AlertTriangle,
  Route,
  OctagonX,
  ArrowUpRight,
} from 'lucide-react';

const easeOutQuart = [0.25, 1, 0.5, 1] as [number, number, number, number];

const stagger = {
  container: { transition: { staggerChildren: 0.06 } },
  item: {
    hidden: { opacity: 0, y: 12 },
    visible: { opacity: 1, y: 0, transition: { duration: 0.4, ease: easeOutQuart } },
  },
};

function StatCard({ icon: Icon, label, value, variant }: {
  icon: any; label: string; value: number; variant?: 'default' | 'danger' | 'warning';
}) {
  return (
    <motion.div
      variants={stagger.item}
      className="group relative rounded-lg border border-border bg-card p-4 transition-colors hover:border-foreground/15"
    >
      <div className="flex items-center justify-between">
        <div className="flex h-9 w-9 items-center justify-center rounded-md bg-muted">
          <Icon className={`h-4 w-4 ${
            variant === 'danger' ? 'text-destructive' :
            variant === 'warning' ? 'text-warning' :
            'text-foreground/70'
          }`} />
        </div>
        <p className={`text-2xl font-bold tracking-tight tabular-nums ${
          variant === 'danger' && value > 0 ? 'text-destructive' :
          variant === 'warning' && value > 0 ? 'text-warning' :
          'text-foreground'
        }`}>{value}</p>
      </div>
      <p className="mt-3 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">{label}</p>
    </motion.div>
  );
}

export default function DashboardPage() {
  const { instances, restartInstance, stopInstance, startInstance, isPending, getPendingAction } = useInstanceControl();
  const { data: health } = useQuery({ queryKey: ['health'], queryFn: api.health });
  const { data: routes = [] } = useQuery({ queryKey: ['routes'], queryFn: api.routes });
  const { data: logs = [] } = useQuery({ queryKey: ['logs'], queryFn: () => api.logs() });
  const stats = health?.counts;

  return (
    <DashboardLayout>
      <div className="p-8 space-y-8 max-w-[1400px]">
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
        >
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Overview</h1>
          <p className="mt-1 text-sm text-muted-foreground">Your local development environment at a glance</p>
        </motion.div>

        <motion.div
          className="grid grid-cols-2 gap-3 lg:grid-cols-3 xl:grid-cols-6"
          initial="hidden"
          animate="visible"
          variants={stagger.container}
        >
          <StatCard icon={FolderKanban} label="Projects" value={stats?.totalProjects ?? 0} />
          <StatCard icon={GitBranch} label="Workspaces" value={stats?.activeWorkspaces ?? 0} />
          <StatCard icon={Play} label="Running" value={stats?.runningInstances ?? 0} />
          <StatCard icon={AlertTriangle} label="Unhealthy" value={stats?.unhealthyInstances ?? 0} variant="danger" />
          <StatCard icon={Route} label="Active Routes" value={stats?.activeRoutes ?? 0} />
          <StatCard icon={OctagonX} label="Conflicts" value={stats?.conflictRoutes ?? 0} variant="warning" />
        </motion.div>

        <div className="grid gap-6 lg:grid-cols-2">
          <motion.div
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.3, duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
            className="rounded-lg border border-border bg-card"
          >
            <div className="flex items-center justify-between px-5 py-4">
              <h2 className="text-sm font-semibold text-foreground">Service Instances</h2>
              <span className="text-[11px] font-mono text-muted-foreground">{instances.length} total</span>
            </div>
            <div className="border-t border-border">
              {instances.map((inst) => (
                <Link
                  key={inst.id}
                  to={`/projects/${inst.projectId}`}
                  className="group flex items-center justify-between px-5 py-3 transition-colors hover:bg-accent/40 border-b border-border last:border-b-0"
                >
                  <div className="flex items-center gap-3">
                    <StatusDot status={inst.status} reason={inst.statusReason} />
                    <div>
                      <p className="text-[13px] font-medium text-foreground">
                        <span className="text-muted-foreground">{inst.projectName}/</span>
                        {inst.serviceName}
                      </p>
                      <p className="text-[11px] text-muted-foreground/70">{inst.workspaceName}</p>
                    </div>
                  </div>
                  <div className="flex items-center gap-4">
                    {inst.port > 0 && (
                      <span className="font-mono text-[11px] text-muted-foreground">:{inst.port}</span>
                    )}
                    <span className="text-[11px] text-muted-foreground/60">{inst.uptime}</span>
                    <StatusBadge status={inst.status} reason={inst.statusReason} />
                    <InstanceActions
                      instance={inst}
                      isPending={isPending(inst.id)}
                      pendingAction={getPendingAction(inst.id)}
                      onRestart={restartInstance}
                      onStop={stopInstance}
                      onStart={startInstance}
                    />
                  </div>
                </Link>
              ))}
            </div>
          </motion.div>

          <motion.div
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.4, duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
            className="rounded-lg border border-border bg-card"
          >
            <div className="flex items-center justify-between px-5 py-4">
              <h2 className="text-sm font-semibold text-foreground">Recent Logs</h2>
              <Link to="/logs" className="inline-flex items-center gap-1 text-[11px] font-medium text-foreground/60 hover:text-foreground transition-colors">
                View all <ArrowUpRight className="h-3 w-3" />
              </Link>
            </div>
            <div className="border-t border-border font-mono text-[12px]">
              {logs.slice(0, 10).map((log, i) => (
                <div key={`${log.timestamp}-${i}`} className="flex gap-3 px-5 py-2.5 border-b border-border last:border-b-0 hover:bg-accent/30 transition-colors">
                  <span className="text-muted-foreground/50 shrink-0 w-[52px] tabular-nums">
                    {new Date(log.timestamp).toLocaleTimeString('en', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                  </span>
                  <span className={`shrink-0 w-[40px] font-semibold ${
                    log.level === 'error' ? 'text-destructive' :
                    log.level === 'warn' ? 'text-warning' :
                    log.level === 'debug' ? 'text-muted-foreground/50' :
                    'text-muted-foreground'
                  }`}>
                    {log.level.toUpperCase()}
                  </span>
                  <span className="text-muted-foreground/40 shrink-0">[{log.source}]</span>
                  <span className="text-foreground/80 truncate">{log.message}</span>
                </div>
              ))}
            </div>
          </motion.div>
        </div>

        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.5, duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
          className="rounded-lg border border-border bg-card"
        >
          <div className="flex items-center justify-between px-5 py-4">
            <h2 className="text-sm font-semibold text-foreground">Routes</h2>
            <Link to="/routes" className="inline-flex items-center gap-1 text-[11px] font-medium text-foreground/60 hover:text-foreground transition-colors">
              View all <ArrowUpRight className="h-3 w-3" />
            </Link>
          </div>
          <div className="border-t border-border overflow-x-auto">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="border-b border-border">
                  <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/60">Pattern</th>
                  <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/60">Target</th>
                  <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/60">Service</th>
                  <th className="px-5 py-2.5 text-left text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/60">Status</th>
                </tr>
              </thead>
              <tbody>
                {routes.slice(0, 10).map((route) => (
                  <tr key={route.id} className="border-b border-border last:border-b-0 hover:bg-accent/30 transition-colors">
                    <td className="px-5 py-2.5 font-mono text-[12px] text-foreground">{route.pattern}</td>
                    <td className="px-5 py-2.5 font-mono text-[12px] text-muted-foreground">{route.target}</td>
                    <td className="px-5 py-2.5 text-muted-foreground">
                      <span className="text-foreground/70">{route.projectName}</span>
                      <span className="text-muted-foreground/40">/</span>
                      <span>{route.serviceName}</span>
                    </td>
                    <td className="px-5 py-2.5"><RouteStatusBadge status={route.status} reason={route.conflictReason} /></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </motion.div>
      </div>
    </DashboardLayout>
  );
}
