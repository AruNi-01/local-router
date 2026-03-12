import { Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { DashboardLayout } from '@/components/DashboardLayout';
import { api } from '@/lib/api';
import { StatusDot } from '@/components/StatusIndicators';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { FolderOpen, GitBranch, Box, ArrowUpRight, Plus, RefreshCw, Search, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { toast } from 'sonner';
import type { HealthStatus, Instance } from '@/lib/types';

function getProjectHealth(projectId: string, instances: Instance[]): HealthStatus {
  const projectInstances = instances.filter(i => i.projectId === projectId);
  if (projectInstances.some(i => i.status === 'unhealthy')) return 'unhealthy';
  if (projectInstances.some(i => i.status === 'starting')) return 'starting';
  if (projectInstances.length > 0 && projectInstances.every(i => i.status === 'stopped')) return 'stopped';
  return 'healthy';
}

const easeOutQuart = [0.25, 1, 0.5, 1] as [number, number, number, number];

const stagger = {
  container: { transition: { staggerChildren: 0.08 } },
  item: {
    hidden: { opacity: 0, y: 16 },
    visible: { opacity: 1, y: 0, transition: { duration: 0.45, ease: easeOutQuart } },
  },
};

export default function ProjectsPage() {
  const queryClient = useQueryClient();
  const [projectPath, setProjectPath] = useState('');
  const [search, setSearch] = useState('');
  const { data: projects = [] } = useQuery({ queryKey: ['projects'], queryFn: api.projects });
  const { data: workspaces = [] } = useQuery({ queryKey: ['workspaces'], queryFn: api.workspaces });
  const { data: services = [] } = useQuery({ queryKey: ['services'], queryFn: api.services });
  const { data: instances = [] } = useQuery({ queryKey: ['instances'], queryFn: api.instances });
  const addProject = useMutation({
    mutationFn: api.addProject,
    onSuccess: () => {
      setProjectPath('');
      toast.success('Project imported');
      queryClient.invalidateQueries();
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Failed to import project');
    },
  });
  const rescanProject = useMutation({
    mutationFn: api.rescanProject,
    onSuccess: () => {
      toast.success('Project rescanned');
      queryClient.invalidateQueries();
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Failed to rescan project');
    },
  });
  const deleteProject = useMutation({
    mutationFn: api.deleteProject,
    onSuccess: () => {
      toast.success('Project removed');
      queryClient.invalidateQueries();
    },
    onError: (error) => {
      toast.error(error instanceof Error ? error.message : 'Failed to remove project');
    },
  });
  const filteredProjects = projects.filter((project) => {
    const query = search.trim().toLowerCase();
    if (!query) {
      return true;
    }
    const projectWorkspaces = workspaces.filter((workspace) => workspace.projectId === project.id);
    const projectServices = services.filter((service) => service.projectId === project.id);
    return [
      project.name,
      project.path,
      project.configSource ?? '',
      ...projectWorkspaces.flatMap((workspace) => [workspace.name, workspace.branch, workspace.path]),
      ...projectServices.flatMap((service) => [service.name, service.adapter, service.route]),
    ].some((value) => value.toLowerCase().includes(query));
  });

  return (
    <DashboardLayout>
      <div className="p-8 space-y-8 max-w-[1400px]">
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
        >
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Projects</h1>
          <p className="mt-1 text-sm text-muted-foreground">Registered local projects</p>
        </motion.div>

        <div className="flex flex-col gap-3 rounded-lg border border-border bg-card p-4 md:flex-row md:items-end">
          <div className="flex-1 space-y-2">
            <p className="text-xs font-semibold uppercase tracking-widest text-muted-foreground/60">Import Project</p>
            <Input
              value={projectPath}
              onChange={(e) => setProjectPath(e.target.value)}
              placeholder="/absolute/path/to/repo"
            />
          </div>
          <Button
            onClick={() => projectPath.trim() && addProject.mutate(projectPath.trim())}
            disabled={addProject.isPending || !projectPath.trim()}
          >
            <Plus className="mr-2 h-4 w-4" />
            Add Project
          </Button>
        </div>

        <div className="relative max-w-md">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground/50" />
          <Input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="Search project, path, branch, service"
            className="pl-9"
          />
        </div>

        <motion.div
          className="grid gap-4 md:grid-cols-2 xl:grid-cols-3"
          initial="hidden"
          animate="visible"
          variants={stagger.container}
        >
          {filteredProjects.map((project) => {
            const projectWorkspaces = workspaces.filter(w => w.projectId === project.id);
            const projectServices = services.filter(s => s.projectId === project.id);
            const projectInstances = instances.filter(i => i.projectId === project.id);
            const running = projectInstances.filter(i => i.status === 'healthy' || i.status === 'starting').length;
            const health = getProjectHealth(project.id, instances);

            return (
              <motion.div key={project.id} variants={stagger.item}>
                <div className="group rounded-lg border border-border bg-card p-5 transition-all duration-300 hover:border-foreground/20">
                  <Link to={`/projects/${project.id}`} className="block">
                    <div className="flex items-start justify-between">
                      <div className="flex items-center gap-3">
                        <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-muted">
                          <FolderOpen className="h-5 w-5 text-foreground/60" />
                        </div>
                        <div>
                          <h3 className="text-[14px] font-semibold text-foreground group-hover:text-foreground/80 transition-colors duration-200">
                            {project.name}
                          </h3>
                          <p className="text-[11px] font-mono text-muted-foreground/60">{project.path}</p>
                        </div>
                      </div>
                      <div className="flex items-center gap-2">
                        <StatusDot status={health} />
                        <ArrowUpRight className="h-3.5 w-3.5 text-muted-foreground/30 group-hover:text-foreground/50 transition-colors" />
                      </div>
                    </div>

                    <div className="mt-5 flex gap-5 text-[11px] text-muted-foreground">
                      <span className="flex items-center gap-1.5">
                        <GitBranch className="h-3 w-3 text-muted-foreground/40" />
                        {projectWorkspaces.length} workspaces
                      </span>
                      <span className="flex items-center gap-1.5">
                        <Box className="h-3 w-3 text-muted-foreground/40" />
                        {projectServices.length} services
                      </span>
                      <span className="font-medium text-foreground/60">{running} running</span>
                    </div>
                  </Link>

                  <div className="mt-4 flex items-center justify-end gap-2 border-t border-border pt-4">
                    <Button
                      variant="ghost"
                      size="sm"
                      disabled={rescanProject.isPending}
                      onClick={() => rescanProject.mutate(project.id)}
                    >
                      <RefreshCw className="mr-2 h-3.5 w-3.5" />
                      Rescan
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      className="text-destructive hover:text-destructive"
                      disabled={deleteProject.isPending}
                      onClick={() => {
                        if (window.confirm(`Remove project "${project.name}" from LocalRouter?`)) {
                          deleteProject.mutate(project.id);
                        }
                      }}
                    >
                      <Trash2 className="mr-2 h-3.5 w-3.5" />
                      Remove
                    </Button>
                  </div>
                </div>
              </motion.div>
            );
          })}
        </motion.div>
        {filteredProjects.length === 0 && (
          <div className="rounded-lg border border-dashed border-border bg-card px-5 py-6 text-sm text-muted-foreground">
            No projects match the current search.
          </div>
        )}
      </div>
    </DashboardLayout>
  );
}
