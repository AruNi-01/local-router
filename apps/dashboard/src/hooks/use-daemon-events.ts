import { useEffect } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { api } from '@/lib/api';

function eventsUrl() {
  const url = new URL(api.apiUrl('/events'));
  url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
  return url.toString();
}

export function useDaemonEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    const socket = new WebSocket(eventsUrl());
    socket.onmessage = () => {
      queryClient.invalidateQueries();
    };
    return () => socket.close();
  }, [queryClient]);
}
