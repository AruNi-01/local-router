import { useEffect } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';

function eventsUrl() {
  const url = new URL(api.apiUrl('/events'));
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

export function useDaemonEvents() {
  const queryClient = useQueryClient();
  const { data: config } = useQuery({
    queryKey: ['config'],
    queryFn: api.config,
  });

  useEffect(() => {
    if (!config?.hotReload) {
      return;
    }

    const socket = new WebSocket(eventsUrl());
    socket.onmessage = () => {
      queryClient.invalidateQueries();
    };
    return () => socket.close();
  }, [config?.hotReload, queryClient]);
}
