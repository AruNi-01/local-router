import { motion } from 'framer-motion';
import { useQuery } from '@tanstack/react-query';
import { DashboardLayout } from '@/components/DashboardLayout';
import { Input } from '@/components/ui/input';
import { api } from '@/lib/api';
import { useState } from 'react';
import { cn } from '@/lib/utils';
import { Search } from 'lucide-react';

const levels = ['all', 'info', 'warn', 'error', 'debug'] as const;

export default function LogsPage() {
  const [levelFilter, setLevelFilter] = useState<typeof levels[number]>('all');
  const [search, setSearch] = useState('');
  const { data: logs = [] } = useQuery({ queryKey: ['logs'], queryFn: () => api.logs() });
  const filtered = logs.filter(log => {
    if (levelFilter !== 'all' && log.level !== levelFilter) {
      return false;
    }
    const query = search.trim().toLowerCase();
    if (!query) {
      return true;
    }
    return [log.source, log.message, log.level].some(value => value.toLowerCase().includes(query));
  });

  return (
    <DashboardLayout>
      <div className="p-8 space-y-8 max-w-[1400px]">
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
        >
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Logs</h1>
          <p className="mt-1 text-sm text-muted-foreground">Aggregated service logs</p>
        </motion.div>

        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ delay: 0.15, duration: 0.4 }}
          className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between"
        >
          <div className="flex gap-1.5 rounded-lg bg-muted p-1" style={{ width: 'fit-content' }}>
            {levels.map(f => (
              <button
                key={f}
                onClick={() => setLevelFilter(f)}
                className={cn(
                  'relative rounded-md px-3 py-1.5 text-[12px] font-medium transition-all duration-200',
                  levelFilter === f
                    ? 'text-foreground'
                    : 'text-muted-foreground hover:text-foreground/70'
                )}
              >
                {levelFilter === f && (
                  <motion.div
                    layoutId="log-filter"
                    className="absolute inset-0 rounded-md bg-accent border border-border"
                    transition={{ type: 'spring', stiffness: 400, damping: 30 }}
                  />
                )}
                <span className="relative">{f.toUpperCase()}</span>
              </button>
            ))}
          </div>
          <label className="relative block w-full max-w-sm">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground/50" />
            <Input
              value={search}
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search source, level, message"
              className="pl-9 text-[13px]"
            />
          </label>
        </motion.div>

        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.25, duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
          className="rounded-lg border border-border bg-card overflow-hidden"
        >
          <div className="font-mono text-[12px]">
            {filtered.map((log, i) => (
              <motion.div
                key={`${log.timestamp}-${i}`}
                initial={{ opacity: 0, x: -4 }}
                animate={{ opacity: 1, x: 0 }}
                transition={{ delay: i * 0.02, duration: 0.3, ease: [0.25, 1, 0.5, 1] }}
                className="flex gap-4 px-5 py-2.5 border-b border-border last:border-b-0 hover:bg-accent/30 transition-colors"
              >
                <span className="text-muted-foreground/40 shrink-0 w-[56px] tabular-nums">
                  {new Date(log.timestamp).toLocaleTimeString('en', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                </span>
                <span className={cn(
                  'shrink-0 w-[44px] font-semibold',
                  log.level === 'error' && 'text-destructive',
                  log.level === 'warn' && 'text-warning',
                  log.level === 'debug' && 'text-muted-foreground/40',
                  log.level === 'info' && 'text-muted-foreground',
                )}>
                  {log.level.toUpperCase()}
                </span>
                <span className="text-muted-foreground/30 shrink-0">[{log.source}]</span>
                <span className="text-foreground/80">{log.message}</span>
              </motion.div>
            ))}
            {filtered.length === 0 && (
              <div className="px-5 py-10 text-center font-sans text-sm text-muted-foreground">
                No logs match the current filter.
              </div>
            )}
          </div>
        </motion.div>
      </div>
    </DashboardLayout>
  );
}
