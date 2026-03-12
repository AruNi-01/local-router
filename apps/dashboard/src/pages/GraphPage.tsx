import { motion } from 'framer-motion';
import { useQuery } from '@tanstack/react-query';
import { DashboardLayout } from '@/components/DashboardLayout';
import { api } from '@/lib/api';
import { useEffect, useRef, useCallback, useState } from 'react';
import type { GraphNode } from '@/lib/types';

const nodeColors: Record<GraphNode['type'], { fill: string; stroke: string; label: string }> = {
  project:   { fill: 'hsl(0, 0%, 80%)', stroke: 'hsl(0, 0%, 60%)', label: 'Project' },
  workspace: { fill: 'hsl(0, 0%, 65%)', stroke: 'hsl(0, 0%, 50%)', label: 'Workspace' },
  service:   { fill: 'hsl(0, 0%, 55%)', stroke: 'hsl(0, 0%, 40%)', label: 'Service' },
  route:     { fill: 'hsl(0, 0%, 45%)', stroke: 'hsl(0, 0%, 35%)', label: 'Route' },
};

const statusFills: Record<string, string> = {
  healthy: 'hsl(142, 71%, 45%)',
  unhealthy: 'hsl(0, 72%, 51%)',
  starting: 'hsl(45, 93%, 47%)',
  stopped: 'hsl(0, 0%, 30%)',
  unknown: 'hsl(0, 0%, 35%)',
};

const statusLabels: Record<string, string> = {
  healthy: 'Healthy',
  unhealthy: 'Unhealthy',
  starting: 'Starting',
  stopped: 'Stopped',
  unknown: 'Unknown',
};

function getNodeRadius(type: GraphNode['type']) {
  return type === 'project' ? 22 : type === 'workspace' ? 18 : type === 'route' ? 12 : 15;
}

export default function GraphPage() {
  const { data: graph } = useQuery({ queryKey: ['graph'], queryFn: api.graph });
  const nodes = graph?.nodes ?? [];
  const edges = graph?.edges ?? [];
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const positionsRef = useRef<Record<string, { x: number; y: number }>>({});
  const initializedRef = useRef(false);
  const [dragId, setDragId] = useState<string | null>(null);
  const [hoverId, setHoverId] = useState<string | null>(null);
  const dragOffsetRef = useRef({ dx: 0, dy: 0 });

  const initPositions = useCallback((w: number, h: number) => {
    if (initializedRef.current && Object.keys(positionsRef.current).length > 0) return;
    const layers: Record<GraphNode['type'], number> = { project: 0, workspace: 1, service: 2, route: 3 };
    const layerGroups: Record<number, GraphNode[]> = {};
    nodes.forEach((node) => {
      const layer = layers[node.type];
      if (!layerGroups[layer]) layerGroups[layer] = [];
      layerGroups[layer].push(node);
    });
    const layerY = [h * 0.12, h * 0.34, h * 0.56, h * 0.78];
    Object.entries(layerGroups).forEach(([layerStr, group]) => {
      const layer = parseInt(layerStr, 10);
      const spacing = w / (group.length + 1);
      group.forEach((node, i) => {
        positionsRef.current[node.id] = { x: spacing * (i + 1), y: layerY[layer] };
      });
    });
    initializedRef.current = true;
  }, [nodes]);

  const draw = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    ctx.scale(dpr, dpr);
    const w = rect.width;
    const h = rect.height;

    initPositions(w, h);
    const positions = positionsRef.current;

    ctx.fillStyle = 'hsl(0, 0%, 8%)';
    ctx.fillRect(0, 0, w, h);
    ctx.strokeStyle = 'hsl(0, 0%, 12%)';
    ctx.lineWidth = 0.5;
    for (let x = 0; x < w; x += 48) {
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, h);
      ctx.stroke();
    }
    for (let y = 0; y < h; y += 48) {
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(w, y);
      ctx.stroke();
    }

    edges.forEach((edge) => {
      const from = positions[edge.source];
      const to = positions[edge.target];
      if (!from || !to) return;
      const isHovered = hoverId === edge.source || hoverId === edge.target;
      ctx.beginPath();
      ctx.strokeStyle = edge.type === 'depends_on'
        ? (isHovered ? 'hsla(0, 72%, 51%, 0.6)' : 'hsla(0, 72%, 51%, 0.3)')
        : (isHovered ? 'hsla(0, 0%, 95%, 0.2)' : 'hsla(0, 0%, 95%, 0.06)');
      ctx.lineWidth = edge.type === 'depends_on' ? 1.5 : 1;
      ctx.setLineDash(edge.type === 'depends_on' ? [4, 4] : []);
      const midY = (from.y + to.y) / 2;
      ctx.moveTo(from.x, from.y);
      ctx.bezierCurveTo(from.x, midY, to.x, midY, to.x, to.y);
      ctx.stroke();
      ctx.setLineDash([]);
    });

    nodes.forEach((node) => {
      const pos = positions[node.id];
      if (!pos) return;
      const colors = nodeColors[node.type];
      const r = getNodeRadius(node.type);
      const fillColor = node.status ? (statusFills[node.status] || colors.fill) : colors.fill;
      const isHovered = hoverId === node.id;
      const isDragged = dragId === node.id;

      if (isHovered || isDragged) {
        ctx.beginPath();
        ctx.arc(pos.x, pos.y, r + 6, 0, Math.PI * 2);
        ctx.strokeStyle = 'hsla(0, 0%, 95%, 0.15)';
        ctx.lineWidth = 1;
        ctx.stroke();
      }

      ctx.beginPath();
      ctx.arc(pos.x, pos.y, r, 0, Math.PI * 2);
      ctx.fillStyle = fillColor;
      ctx.globalAlpha = isHovered || isDragged ? 0.3 : 0.15;
      ctx.fill();
      ctx.globalAlpha = 1;
      ctx.strokeStyle = fillColor;
      ctx.lineWidth = isHovered || isDragged ? 2 : 1.5;
      ctx.stroke();

      ctx.beginPath();
      ctx.arc(pos.x, pos.y, 3, 0, Math.PI * 2);
      ctx.fillStyle = fillColor;
      ctx.globalAlpha = 0.6;
      ctx.fill();
      ctx.globalAlpha = 1;

      ctx.fillStyle = isHovered ? 'hsl(0, 0%, 80%)' : 'hsl(0, 0%, 55%)';
      ctx.font = '500 11px "Plus Jakarta Sans", system-ui, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(node.label, pos.x, pos.y + r + 18);
    });

    if (hoverId && !dragId) {
      const node = nodes.find((item) => item.id === hoverId);
      if (node) {
        const pos = positions[node.id];
        if (pos) {
          const colors = nodeColors[node.type];
          const lines = [
            `${colors.label}: ${node.label}`,
            ...(node.status ? [`Status: ${statusLabels[node.status] || node.status}`] : []),
            `ID: ${node.id}`,
            `Connections: ${edges.filter((edge) => edge.source === node.id || edge.target === node.id).length}`,
          ];
          const padding = 12;
          const lineHeight = 18;
          const tooltipW = 220;
          const tooltipH = padding * 2 + lines.length * lineHeight;
          let tx = pos.x + 30;
          let ty = pos.y - tooltipH / 2;
          if (tx + tooltipW > rect.width) tx = pos.x - tooltipW - 30;
          if (ty < 8) ty = 8;
          if (ty + tooltipH > rect.height - 8) ty = rect.height - tooltipH - 8;

          ctx.fillStyle = 'hsl(0, 0%, 12%)';
          ctx.strokeStyle = 'hsl(0, 0%, 20%)';
          ctx.lineWidth = 1;
          roundRect(ctx, tx, ty, tooltipW, tooltipH, 6);
          ctx.fill();
          ctx.stroke();

          ctx.textAlign = 'left';
          lines.forEach((line, index) => {
            ctx.fillStyle = index === 0 ? 'hsl(0, 0%, 90%)' : 'hsl(0, 0%, 55%)';
            ctx.fillText(line, tx + padding, ty + padding + 12 + index * lineHeight);
          });
        }
      }
    }
  }, [dragId, edges, hoverId, initPositions, nodes]);

  function roundRect(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + r);
    ctx.lineTo(x + w, y + h - r);
    ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
    ctx.lineTo(x + r, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - r);
    ctx.lineTo(x, y + r);
    ctx.quadraticCurveTo(x, y, x + r, y);
    ctx.closePath();
  }

  function hitTest(mx: number, my: number): string | null {
    for (const node of nodes) {
      const pos = positionsRef.current[node.id];
      if (!pos) continue;
      const r = getNodeRadius(node.type) + 6;
      const dx = mx - pos.x;
      const dy = my - pos.y;
      if (dx * dx + dy * dy <= r * r) return node.id;
    }
    return null;
  }

  function getCanvasCoords(e: React.MouseEvent<HTMLCanvasElement>) {
    const rect = canvasRef.current!.getBoundingClientRect();
    return { x: e.clientX - rect.left, y: e.clientY - rect.top };
  }

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const { x, y } = getCanvasCoords(e);
    const id = hitTest(x, y);
    if (id) {
      const pos = positionsRef.current[id];
      dragOffsetRef.current = { dx: x - pos.x, dy: y - pos.y };
      setDragId(id);
    }
  }, [nodes]);

  const handleMouseMove = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const { x, y } = getCanvasCoords(e);
    if (dragId) {
      positionsRef.current[dragId] = {
        x: x - dragOffsetRef.current.dx,
        y: y - dragOffsetRef.current.dy,
      };
      draw();
      return;
    }
    const id = hitTest(x, y);
    setHoverId(id);
    if (canvasRef.current) {
      canvasRef.current.style.cursor = id ? 'grab' : 'default';
    }
  }, [dragId, draw, nodes]);

  const handleMouseUp = useCallback(() => {
    setDragId(null);
  }, []);

  useEffect(() => {
    initializedRef.current = false;
    positionsRef.current = {};
    draw();
    const handleResize = () => draw();
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [draw]);

  return (
    <DashboardLayout>
      <div className="p-8 space-y-6 h-full flex flex-col max-w-[1400px]">
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
        >
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Topology</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Drag nodes to rearrange · Hover for details
          </p>
        </motion.div>
        <motion.div
          initial={{ opacity: 0, scale: 0.98 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: 0.2, duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
          className="flex-1 rounded-lg border border-border overflow-hidden min-h-[500px]"
        >
          <canvas
            ref={canvasRef}
            className="w-full h-full"
            style={{ minHeight: 500 }}
            onMouseDown={handleMouseDown}
            onMouseMove={handleMouseMove}
            onMouseUp={handleMouseUp}
            onMouseLeave={() => { setHoverId(null); setDragId(null); }}
          />
        </motion.div>
      </div>
    </DashboardLayout>
  );
}
