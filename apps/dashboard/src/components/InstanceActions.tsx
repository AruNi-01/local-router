import { Play, Square, RotateCw, Loader2 } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import type { Instance } from '@/lib/types';

interface Props {
  instance: Instance;
  isPending: boolean;
  pendingAction: string | null;
  onRestart: (inst: Instance) => void;
  onStop: (inst: Instance) => void;
  onStart: (inst: Instance) => void;
}

export function InstanceActions({ instance, isPending, pendingAction, onRestart, onStop, onStart }: Props) {
  const isStopped = instance.status === 'stopped';

  if (isPending) {
    return (
      <div className="flex items-center gap-1 text-muted-foreground">
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
        <span className="text-[10px] capitalize">{pendingAction}…</span>
      </div>
    );
  }

  return (
    <div className="flex items-center gap-0.5">
      {isStopped ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 w-7 p-0 text-muted-foreground hover:text-success"
              onClick={(e) => { e.preventDefault(); e.stopPropagation(); onStart(instance); }}
            >
              <Play className="h-3.5 w-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="top">Start</TooltipContent>
        </Tooltip>
      ) : (
        <>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 w-7 p-0 text-muted-foreground hover:text-foreground"
                onClick={(e) => { e.preventDefault(); e.stopPropagation(); onRestart(instance); }}
              >
                <RotateCw className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top">Restart</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 w-7 p-0 text-muted-foreground hover:text-destructive"
                onClick={(e) => { e.preventDefault(); e.stopPropagation(); onStop(instance); }}
              >
                <Square className="h-3.5 w-3.5" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top">Stop</TooltipContent>
          </Tooltip>
        </>
      )}
    </div>
  );
}
