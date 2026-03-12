import { motion } from 'framer-motion';
import { useQuery } from '@tanstack/react-query';
import { DashboardLayout } from '@/components/DashboardLayout';
import { RouteStatusBadge } from '@/components/StatusIndicators';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { api } from '@/lib/api';
import { copyText } from '@/lib/clipboard';
import { useState } from 'react';
import { cn } from '@/lib/utils';
import { Copy, ExternalLink, Search } from 'lucide-react';

const filters = ['all', 'active', 'stale', 'conflict'] as const;

export default function RoutesPage() {
  const [filter, setFilter] = useState<typeof filters[number]>('all');
  const [search, setSearch] = useState('');
  const { data: routes = [] } = useQuery({ queryKey: ['routes'], queryFn: api.routes });
  const filtered = routes.filter(route => {
    if (filter !== 'all' && route.status !== filter) {
      return false;
    }
    const query = search.trim().toLowerCase();
    if (!query) {
      return true;
    }
    return [
      route.pattern,
      route.url,
      route.target,
      route.projectName,
      route.serviceName,
      route.workspaceName,
      route.conflictReason ?? '',
    ].some(value => value.toLowerCase().includes(query));
  });

  return (
    <DashboardLayout>
      <div className="p-8 space-y-8 max-w-[1400px]">
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
        >
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Routes</h1>
          <p className="mt-1 text-sm text-muted-foreground">All registered proxy routes</p>
        </motion.div>

        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.15, duration: 0.4 }}
          className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between"
        >
          <div className="flex gap-1.5 rounded-lg bg-muted p-1" style={{ width: 'fit-content' }}>
            {filters.map(f => (
              <button
                key={f}
                onClick={() => setFilter(f)}
                className={cn(
                  'relative rounded-md px-3 py-1.5 text-[12px] font-medium transition-all duration-200',
                  filter === f
                    ? 'text-foreground'
                    : 'text-muted-foreground hover:text-foreground/70'
                )}
              >
                {filter === f && (
                  <motion.div
                    layoutId="route-filter"
                    className="absolute inset-0 rounded-md bg-accent border border-border"
                    transition={{ type: 'spring', stiffness: 400, damping: 30 }}
                  />
                )}
                <span className="relative">
                  {f.charAt(0).toUpperCase() + f.slice(1)}
                  {f !== 'all' && (
                    <span className="ml-1 text-muted-foreground/50">
                      {routes.filter(r => r.status === f).length}
                    </span>
                  )}
                </span>
              </button>
            ))}
          </div>
          <label className="relative block w-full max-w-sm">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground/50" />
            <Input
              value={search}
              onChange={event => setSearch(event.target.value)}
              placeholder="Search host, project, service, workspace"
              className="pl-9 text-[13px]"
            />
          </label>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.25, duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
          className="rounded-lg border border-border bg-card overflow-x-auto"
        >
          <table className="w-full text-[13px]">
            <thead>
              <tr className="border-b border-border">
                {['Pattern', 'Target', 'Project', 'Service', 'Workspace', 'Status', ''].map(h => (
                  <th key={h} className="px-5 py-3 text-left text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/50">{h}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {filtered.map((route, i) => (
                <motion.tr
                  key={route.id}
                  initial={{ opacity: 0 }}
                  animate={{ opacity: 1 }}
                  transition={{ delay: i * 0.03 }}
                  className="border-b border-border last:border-b-0 hover:bg-accent/30 transition-colors"
                >
                  <td className="px-5 py-3 font-mono text-[12px] text-foreground">
                    <div>{route.pattern}</div>
                    <div className="mt-1 text-[11px] text-muted-foreground/60">{route.url}</div>
                  </td>
                  <td className="px-5 py-3 font-mono text-[12px] text-muted-foreground">{route.target}</td>
                  <td className="px-5 py-3 text-muted-foreground">{route.projectName}</td>
                  <td className="px-5 py-3 text-foreground/80">{route.serviceName}</td>
                  <td className="px-5 py-3 text-muted-foreground/60">{route.workspaceName}</td>
                  <td className="px-5 py-3"><RouteStatusBadge status={route.status} reason={route.conflictReason} /></td>
                  <td className="px-5 py-3">
                    <div className="flex items-center justify-end gap-1">
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-muted-foreground"
                        onClick={() => copyText(route.url, 'Route URL')}
                      >
                        <Copy className="h-3.5 w-3.5" />
                        Copy
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-muted-foreground"
                        onClick={() => window.open(route.url, '_blank', 'noopener,noreferrer')}
                        disabled={route.status !== 'active'}
                      >
                        <ExternalLink className="h-3.5 w-3.5" />
                        Open
                      </Button>
                    </div>
                  </td>
                </motion.tr>
              ))}
            </tbody>
          </table>
        </motion.div>
        {filtered.length === 0 && (
          <div className="rounded-lg border border-dashed border-border bg-card px-5 py-6 text-sm text-muted-foreground">
            No routes match the current filter.
          </div>
        )}
      </div>
    </DashboardLayout>
  );
}
