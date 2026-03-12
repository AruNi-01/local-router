import { useLocation } from 'react-router-dom';
import { motion } from 'framer-motion';
import {
  LayoutDashboard,
  FolderKanban,
  Route,
  Network,
  Terminal,
  Zap,
  Settings,
} from 'lucide-react';
import { NavLink } from '@/components/NavLink';
import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarFooter,
  SidebarHeader,
  useSidebar,
} from '@/components/ui/sidebar';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';

const navItems = [
  { path: '/', icon: LayoutDashboard, label: 'Overview' },
  { path: '/projects', icon: FolderKanban, label: 'Projects' },
  { path: '/routes', icon: Route, label: 'Routes' },
  { path: '/graph', icon: Network, label: 'Topology' },
  { path: '/logs', icon: Terminal, label: 'Logs' },
  { path: '/settings', icon: Settings, label: 'Settings' },
];

export function AppSidebar() {
  const { state } = useSidebar();
  const collapsed = state === 'collapsed';
  const location = useLocation();

  const isActive = (path: string) =>
    path === '/' ? location.pathname === '/' : location.pathname.startsWith(path);

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <div className="flex items-center gap-3 px-2 py-3">
          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-muted">
            <Zap className="h-4 w-4 text-foreground" />
          </div>
          {!collapsed && (
            <div className="overflow-hidden">
              <h1 className="text-[13px] font-bold tracking-tight text-foreground">LocalRouter</h1>
              <p className="text-[10px] font-mono text-muted-foreground tracking-wide">v0.1.0</p>
            </div>
          )}
        </div>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          {!collapsed && <SidebarGroupLabel className="text-[10px] font-semibold uppercase tracking-widest text-muted-foreground/60">Navigate</SidebarGroupLabel>}
          <SidebarGroupContent>
            <SidebarMenu>
              {navItems.map((item) => {
                const active = isActive(item.path);
                const button = (
                  <SidebarMenuButton asChild isActive={active}>
                    <NavLink
                      to={item.path}
                      end={item.path === '/'}
                      className="relative flex items-center gap-2.5"
                      activeClassName="bg-accent text-foreground font-medium"
                    >
                      {active && (
                        <motion.div
                          layoutId="sidebar-indicator"
                          className="absolute inset-0 rounded-md bg-accent border border-border"
                          transition={{ type: 'spring', stiffness: 350, damping: 30 }}
                        />
                      )}
                      <item.icon className="relative h-4 w-4 shrink-0" />
                      {!collapsed && <span className="relative">{item.label}</span>}
                    </NavLink>
                  </SidebarMenuButton>
                );

                return (
                  <SidebarMenuItem key={item.path}>
                    {collapsed ? (
                      <Tooltip>
                        <TooltipTrigger asChild>{button}</TooltipTrigger>
                        <TooltipContent side="right">{item.label}</TooltipContent>
                      </Tooltip>
                    ) : (
                      button
                    )}
                  </SidebarMenuItem>
                );
              })}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>

      <SidebarFooter>
        <div className="px-2 py-3">
          <div className="flex items-center gap-2">
            <span className="relative flex h-2 w-2 shrink-0">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-success opacity-40" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-success" />
            </span>
            {!collapsed && (
              <span className="text-[11px] font-medium text-muted-foreground">Daemon active</span>
            )}
          </div>
          {!collapsed && (
            <p className="mt-1.5 text-[10px] font-mono text-muted-foreground/60">pid 41000 · :9800</p>
          )}
        </div>
      </SidebarFooter>
    </Sidebar>
  );
}
