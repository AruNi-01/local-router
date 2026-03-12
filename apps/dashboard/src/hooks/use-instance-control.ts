import { useCallback, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { toast } from 'sonner';
import { api } from '@/lib/api';
import type { Instance } from '@/lib/types';

type Action = 'restarting' | 'stopping' | 'starting';

export function useInstanceControl() {
  const queryClient = useQueryClient();
  const [pending, setPending] = useState<Record<string, Action | undefined>>({});

  const { data: instances = [] } = useQuery({
    queryKey: ['instances'],
    queryFn: api.instances,
  });

  const runMutation = useMutation({
    mutationFn: async ({ inst, action }: { inst: Instance; action: Action }) => {
      setPending((prev) => ({ ...prev, [inst.id]: action }));
      if (action === 'starting') return api.startInstance(inst.id);
      if (action === 'stopping') return api.stopInstance(inst.id);
      return api.restartInstance(inst.id);
    },
    onSuccess: (_, { inst, action }) => {
      const verb = action === 'starting' ? 'started' : action === 'stopping' ? 'stopped' : 'restarted';
      toast.success(`${inst.projectName}/${inst.serviceName} ${verb}`);
      queryClient.invalidateQueries();
    },
    onError: (error, { inst, action }) => {
      const verb = action === 'starting' ? 'start' : action === 'stopping' ? 'stop' : 'restart';
      toast.error(`Failed to ${verb} ${inst.projectName}/${inst.serviceName}`, {
        description: error instanceof Error ? error.message : 'Unknown error',
      });
    },
    onSettled: (_, __, { inst }) => {
      setPending((prev) => {
        const next = { ...prev };
        delete next[inst.id];
        return next;
      });
    },
  });

  const mutate = useCallback(
    (inst: Instance, action: Action) => {
      runMutation.mutate({ inst, action });
    },
    [runMutation],
  );

  return {
    instances,
    restartInstance: (inst: Instance) => mutate(inst, 'restarting'),
    stopInstance: (inst: Instance) => mutate(inst, 'stopping'),
    startInstance: (inst: Instance) => mutate(inst, 'starting'),
    isPending: (id: string) => Boolean(pending[id]),
    getPendingAction: (id: string) => pending[id] ?? null,
  };
}
