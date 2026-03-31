import { useParams, Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import { useQuery } from '@tanstack/react-query';
import { DashboardLayout } from '@/components/DashboardLayout';
import { StatusBadge, RouteStatusBadge } from '@/components/StatusIndicators';
import { InstanceActions } from '@/components/InstanceActions';
import { useInstanceControl } from '@/hooks/use-instance-control';
import { api } from '@/lib/api';
import { copyText } from '@/lib/clipboard';
import { Button } from '@/components/ui/button';
import { ArrowLeft, GitBranch, Box, ExternalLink } from 'lucide-react';

const easeOutQuart = [0.25, 1, 0.5, 1] as [number, number, number, number];

const fadeUp = {
  hidden: { opacity: 0, y: 12 },
  visible: (i: number) => ({
    opacity: 1, y: 0,
    transition: { delay: i * 0.08, duration: 0.45, ease: easeOutQuart },
  }),
};

export default function ProjectDetailPage() {
  const { projectId } = useParams();
  const { data } = useQuery({
    queryKey: ['project', projectId],
    queryFn: () => api.project(projectId!),
    enabled: Boolean(projectId),
  });
  const { instances: allInstances, restartInstance, stopInstance, startInstance, isPending, getPendingAction } = useInstanceControl();

  if (!data) {
    return (
      <DashboardLayout>
        <div className="flex items-center justify-center h-full text-muted-foreground">Project not found</div>
      </DashboardLayout>
    );
  }

  const { project, workspaces, services, routes } = data;
  const instances = allInstances.filter(i => i.projectId === project.id);
  const servicesById = new Map(services.map((service) => [service.id, service]));

  return (
    <DashboardLayout>
      <div className="p-8 space-y-8 max-w-[1400px]">
        <motion.div
          initial={{ opacity: 0, x: -12 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.4, ease: [0.25, 1, 0.5, 1] }}
          className="flex items-center gap-4"
        >
          <Link to="/projects" className="rounded-md p-2 hover:bg-accent transition-colors duration-150">
            <ArrowLeft className="h-4 w-4 text-muted-foreground" />
          </Link>
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">{project.name}</h1>
            <p className="text-[12px] font-mono text-muted-foreground/60">{project.path}</p>
          </div>
        </motion.div>

        <motion.section custom={0} initial="hidden" animate="visible" variants={fadeUp}>
          <div className="flex items-center gap-2 mb-3">
            <GitBranch className="h-4 w-4 text-muted-foreground/50" />
            <h2 className="text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/60">Workspaces</h2>
          </div>
          <div className="rounded-lg border border-border bg-card">
            {workspaces.map((ws, i) => {
              const wsInstances = instances.filter(inst => inst.workspaceId === ws.id);
              const running = wsInstances.filter(inst => inst.status === 'healthy').length;
              return (
                <div key={ws.id} className={`flex items-center justify-between px-5 py-3.5 ${i > 0 ? 'border-t border-border' : ''}`}>
                  <div>
                    <p className="text-[13px] font-semibold text-foreground">{ws.name}</p>
                    <p className="text-[11px] font-mono text-muted-foreground/50 mt-0.5">{ws.branch} · {ws.path}</p>
                  </div>
                  <div className="flex items-center gap-3 text-[11px]">
                    <span className="font-mono text-muted-foreground tabular-nums">{running}/{wsInstances.length}</span>
                    <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${ws.isActive ? 'bg-success/10 text-success' : 'bg-muted text-muted-foreground/50'}`}>
                      {ws.isActive ? 'Active' : 'Inactive'}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        </motion.section>

        <motion.section custom={1} initial="hidden" animate="visible" variants={fadeUp}>
          <div className="flex items-center gap-2 mb-3">
            <Box className="h-4 w-4 text-muted-foreground/50" />
            <h2 className="text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/60">Service Definitions</h2>
          </div>
          <div className="rounded-lg border border-border bg-card">
            {services.map((svc, i) => (
              <div key={svc.id} className={`px-5 py-4 ${i > 0 ? 'border-t border-border' : ''}`}>
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <p className="text-[13px] font-semibold text-foreground">{svc.name}</p>
                    <span className={`rounded-full px-2 py-0.5 text-[10px] font-medium ${
                      svc.enabled ? 'bg-success/10 text-success' : 'bg-muted text-muted-foreground'
                    }`}>
                      {svc.enabled ? 'Enabled' : 'Disabled'}
                    </span>
                  </div>
                  <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground">{svc.adapter}</span>
                </div>
                <p className="mt-1.5 text-[12px] font-mono text-muted-foreground/50">{svc.command}</p>
                <div className="mt-2 flex gap-4 text-[11px] text-muted-foreground/60">
                  <span>{svc.protocol}</span>
                  <span className="text-muted-foreground/20">·</span>
                  <span>{svc.route}</span>
                  <span className="text-muted-foreground/20">·</span>
                  <span>{svc.language}</span>
                </div>
              </div>
            ))}
          </div>
        </motion.section>

        <motion.section custom={2} initial="hidden" animate="visible" variants={fadeUp}>
          <h2 className="text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/60 mb-3">Running Instances</h2>
          <div className="rounded-lg border border-border bg-card overflow-x-auto">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="border-b border-border">
                  {['Service', 'Workspace', 'Port', 'PID', 'URL', 'Uptime', 'CPU', 'Mem', 'Status', ''].map(h => (
                    <th key={h} className="px-5 py-2.5 text-left text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/50">{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {instances.map((inst, i) => (
                  <tr key={inst.id} className={`hover:bg-accent/30 transition-colors ${i > 0 ? 'border-t border-border' : ''}`}>
                    <td className="px-5 py-2.5 font-medium text-foreground">
                      <div className="flex items-center gap-2">
                        <span>{inst.serviceName}</span>
                        {servicesById.get(inst.serviceId)?.enabled === false && (
                          <span className="rounded-full bg-muted px-2 py-0.5 text-[10px] font-medium text-muted-foreground">
                            Disabled
                          </span>
                        )}
                      </div>
                    </td>
                    <td className="px-5 py-2.5 text-muted-foreground">{inst.workspaceName}</td>
                    <td className="px-5 py-2.5 font-mono text-[11px] text-muted-foreground tabular-nums">{inst.port || '—'}</td>
                    <td className="px-5 py-2.5 font-mono text-[11px] text-muted-foreground/50 tabular-nums">{inst.pid || '—'}</td>
                    <td className="px-5 py-2.5 font-mono text-[11px]">
                      {inst.url ? (
                        <div className="flex items-center gap-1.5">
                          <a href={inst.url} target="_blank" rel="noreferrer" className="inline-flex items-center gap-1 text-foreground/60 hover:text-foreground">
                            {inst.url} <ExternalLink className="h-3 w-3" />
                          </a>
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            className="h-6 px-2 text-[11px] text-muted-foreground"
                            onClick={() => copyText(inst.url, 'Instance URL')}
                          >
                            Copy
                          </Button>
                        </div>
                      ) : <span className="text-muted-foreground/30">—</span>}
                    </td>
                    <td className="px-5 py-2.5 text-[11px] text-muted-foreground tabular-nums">{inst.uptime}</td>
                    <td className="px-5 py-2.5 text-[11px] text-muted-foreground tabular-nums">{inst.cpu.toFixed(1)}%</td>
                    <td className="px-5 py-2.5 text-[11px] text-muted-foreground tabular-nums">{inst.memory}MB</td>
                    <td className="px-5 py-2.5"><StatusBadge status={inst.status} reason={inst.statusReason} /></td>
                    <td className="px-5 py-2.5">
                      <InstanceActions
                        instance={inst}
                        isPending={isPending(inst.id)}
                        pendingAction={getPendingAction(inst.id)}
                        onRestart={restartInstance}
                        onStop={stopInstance}
                        onStart={startInstance}
                        disabled={servicesById.get(inst.serviceId)?.enabled === false}
                        disabledReason={inst.statusReason}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </motion.section>

        <motion.section custom={3} initial="hidden" animate="visible" variants={fadeUp}>
          <h2 className="text-[11px] font-semibold uppercase tracking-widest text-muted-foreground/60 mb-3">Routes</h2>
          <div className="rounded-lg border border-border bg-card overflow-x-auto">
            <table className="w-full text-[13px]">
              <thead>
                <tr className="border-b border-border">
                  {['Pattern', 'Target', 'Service', 'Status', ''].map(h => (
                    <th key={h} className="px-5 py-2.5 text-left text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/50">{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {routes.map((rt, i) => (
                  <tr key={rt.id} className={`hover:bg-accent/30 transition-colors ${i > 0 ? 'border-t border-border' : ''}`}>
                    <td className="px-5 py-2.5 font-mono text-[12px] text-foreground">
                      <div>{rt.pattern}</div>
                      <div className="mt-1 text-[11px] text-muted-foreground/60">{rt.url}</div>
                    </td>
                    <td className="px-5 py-2.5 font-mono text-[12px] text-muted-foreground">{rt.target}</td>
                    <td className="px-5 py-2.5 text-muted-foreground">{rt.serviceName}[{rt.workspaceName}]</td>
                    <td className="px-5 py-2.5"><RouteStatusBadge status={rt.status} reason={rt.conflictReason} /></td>
                    <td className="px-5 py-2.5">
                      <div className="flex items-center justify-end gap-1">
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="h-7 px-2 text-[11px] text-muted-foreground"
                          onClick={() => copyText(rt.url, 'Route URL')}
                        >
                          Copy
                        </Button>
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="h-7 px-2 text-[11px] text-muted-foreground"
                          onClick={() => window.open(rt.url, '_blank', 'noopener,noreferrer')}
                          disabled={rt.status !== 'active'}
                        >
                          Open
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </motion.section>
      </div>
    </DashboardLayout>
  );
}
