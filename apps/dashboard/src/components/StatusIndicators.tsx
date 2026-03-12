import type { ReactNode } from 'react';
import { HealthStatus } from '@/lib/types';
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

interface StatusDotProps {
  status: HealthStatus;
  size?: 'sm' | 'md';
  showLabel?: boolean;
  reason?: string | null;
}

const statusConfig: Record<HealthStatus, { color: string; label: string }> = {
  healthy: { color: 'bg-success', label: 'Healthy' },
  unhealthy: { color: 'bg-destructive', label: 'Unhealthy' },
  starting: { color: 'bg-warning', label: 'Starting' },
  stopped: { color: 'bg-muted-foreground/40', label: 'Stopped' },
  unknown: { color: 'bg-muted-foreground/40', label: 'Unknown' },
};

function isAbnormalHealthStatus(status: HealthStatus) {
  return status !== 'healthy';
}

function defaultRouteReason(status: 'active' | 'stale' | 'conflict') {
  if (status === 'conflict') {
    return 'Another route currently owns this host.';
  }
  if (status === 'stale') {
    return 'No active target is currently attached to this route.';
  }
  return null;
}

function MaybeTooltip({
  children,
  reason,
}: {
  children: ReactNode;
  reason?: string | null;
}) {
  if (!reason) {
    return <>{children}</>;
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span className="inline-flex">{children}</span>
      </TooltipTrigger>
      <TooltipContent side="top" className="max-w-xs text-[11px] leading-relaxed">
        {reason}
      </TooltipContent>
    </Tooltip>
  );
}

export function StatusDot({ status, size = 'md', showLabel = false, reason }: StatusDotProps) {
  const config = statusConfig[status];
  const dotSize = size === 'sm' ? 'h-1.5 w-1.5' : 'h-2 w-2';

  return (
    <MaybeTooltip reason={isAbnormalHealthStatus(status) ? reason : null}>
      <span className="inline-flex items-center gap-2">
        <span className="relative flex">
          {(status === 'healthy' || status === 'starting') && (
            <span className={cn('absolute inline-flex h-full w-full rounded-full opacity-40 animate-ping', config.color)} />
          )}
          <span className={cn(dotSize, 'relative rounded-full', config.color)} />
        </span>
        {showLabel && <span className="text-xs text-muted-foreground">{config.label}</span>}
      </span>
    </MaybeTooltip>
  );
}

export function StatusBadge({ status, reason }: { status: HealthStatus; reason?: string | null }) {
  const config = statusConfig[status];
  return (
    <MaybeTooltip reason={isAbnormalHealthStatus(status) ? reason : null}>
      <span className={cn(
        'inline-flex items-center gap-1.5 rounded-full px-2 py-0.5 text-[11px] font-medium tracking-wide',
        status === 'healthy' && 'bg-success/10 text-success',
        status === 'unhealthy' && 'bg-destructive/10 text-destructive',
        status === 'starting' && 'bg-warning/10 text-warning',
        status === 'stopped' && 'bg-muted text-muted-foreground',
        status === 'unknown' && 'bg-muted text-muted-foreground',
      )}>
        <span className={cn('h-1.5 w-1.5 rounded-full', config.color)} />
        {config.label}
      </span>
    </MaybeTooltip>
  );
}

export function RouteStatusBadge({
  status,
  reason,
}: {
  status: 'active' | 'stale' | 'conflict';
  reason?: string | null;
}) {
  const tooltipReason = status === 'active' ? null : (reason ?? defaultRouteReason(status));
  return (
    <MaybeTooltip reason={tooltipReason}>
      <span className={cn(
        'inline-flex items-center rounded-full px-2 py-0.5 text-[11px] font-medium tracking-wide',
        status === 'active' && 'bg-success/10 text-success',
        status === 'stale' && 'bg-warning/10 text-warning',
        status === 'conflict' && 'bg-destructive/10 text-destructive',
      )}>
        {status.charAt(0).toUpperCase() + status.slice(1)}
      </span>
    </MaybeTooltip>
  );
}
