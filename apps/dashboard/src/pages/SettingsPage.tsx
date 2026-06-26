import { useEffect, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { parse as parseYaml, stringify as stringifyYaml } from 'yaml';
import { motion } from 'framer-motion';
import { DashboardLayout } from '@/components/DashboardLayout';
import { api } from '@/lib/api';
import { cn } from '@/lib/utils';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { Textarea } from '@/components/ui/textarea';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Badge } from '@/components/ui/badge';
import { Separator } from '@/components/ui/separator';
import {
  Save,
  Plus,
  Trash2,
  FileCode2,
  RefreshCw,
  Server,
  Globe,
  Terminal,
  AlertCircle,
  CheckCircle2,
} from 'lucide-react';
import { toast } from 'sonner';
import type { ServiceDef } from '@/lib/types';

const container = {
  hidden: { opacity: 0 },
  show: { opacity: 1, transition: { staggerChildren: 0.06 } },
};
const item = {
  hidden: { opacity: 0, y: 8 },
  show: { opacity: 1, y: 0, transition: { duration: 0.4, ease: [0.25, 1, 0.5, 1] as [number, number, number, number] } },
};

interface ServiceFormData extends ServiceDef {}

function buildManifest(existingManifestYaml: string, projectName: string, services: ServiceFormData[]) {
  let existing: Record<string, unknown> = {};
  try {
    existing = (parseYaml(existingManifestYaml) as Record<string, unknown>) || {};
  } catch { /* use defaults if existing manifest is invalid */ }

  return stringifyYaml({
    project: projectName,
    workspace: existing.workspace || { strategy: 'git-worktree' },
    proxy: existing.proxy || undefined,
    services: Object.fromEntries(services.map((service) => [
      service.name,
      {
        command: service.command,
        cwd: service.cwd || undefined,
        protocol: service.protocol,
        adapter: service.adapter,
        route: service.route,
        healthcheck: service.healthcheck || undefined,
        env: Object.keys(service.env || {}).length > 0 ? service.env : undefined,
        depends_on: (service.dependsOn || []).length > 0 ? service.dependsOn : undefined,
        language: service.language,
        enabled: service.enabled,
      },
    ])),
  });
}

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState<'project' | 'global' | 'manifest'>('project');
  const { data: projects = [] } = useQuery({ queryKey: ['projects'], queryFn: api.projects });
  const { data: globalConfig } = useQuery({ queryKey: ['config'], queryFn: api.config });
  const [selectedProject, setSelectedProject] = useState<string>('');
  const { data: detail } = useQuery({
    queryKey: ['project', selectedProject],
    queryFn: () => api.project(selectedProject),
    enabled: Boolean(selectedProject),
  });
  const [config, setConfig] = useState(globalConfig);
  const [manifest, setManifest] = useState('');
  const [yamlError, setYamlError] = useState<string | null>(null);
  const [services, setServices] = useState<ServiceFormData[]>([]);

  useEffect(() => {
    if (!selectedProject && projects[0]) {
      setSelectedProject(projects[0].id);
    }
  }, [projects, selectedProject]);

  useEffect(() => {
    if (globalConfig) setConfig(globalConfig);
  }, [globalConfig]);

  useEffect(() => {
    if (!detail) return;
    setManifest(detail.manifest);
    setServices(detail.services.map((service) => ({ ...service })));
  }, [detail]);

  const saveConfig = useMutation({
    mutationFn: api.saveConfig,
    onSuccess: () => {
      toast.success('Daemon config saved');
      queryClient.invalidateQueries({ queryKey: ['config'] });
    },
    onError: (error) => toast.error(error instanceof Error ? error.message : 'Failed to save config'),
  });

  const saveManifest = useMutation({
    mutationFn: ({ projectId, nextManifest }: { projectId: string; nextManifest: string }) =>
      api.saveManifest(projectId, { manifest: nextManifest }),
    onSuccess: (_, variables) => {
      toast.success('Manifest saved');
      queryClient.invalidateQueries({ queryKey: ['project', variables.projectId] });
      queryClient.invalidateQueries({ queryKey: ['services'] });
      queryClient.invalidateQueries({ queryKey: ['routes'] });
      queryClient.invalidateQueries({ queryKey: ['instances'] });
    },
    onError: (error) => toast.error(error instanceof Error ? error.message : 'Failed to save manifest'),
  });

  const onManifestChange = (val: string) => {
    setManifest(val);
    try {
      parseYaml(val, { strict: true });
      setYamlError(null);
    } catch (error: any) {
      setYamlError(error.message?.split('\n')[0] || 'Invalid YAML');
    }
  };

  const handleServiceChange = (index: number, field: keyof ServiceFormData, value: string | boolean) => {
    setServices((prev) => prev.map((service, i) => i === index ? { ...service, [field]: value } : service));
  };

  const addService = () => {
    if (!detail) return;
    setServices((prev) => [
      ...prev,
      {
        id: `svc-new-${Date.now()}`,
        projectId: detail.project.id,
        name: '',
        command: '',
        protocol: 'http',
        adapter: 'generic',
        route: '',
        healthcheck: '',
        language: 'generic',
        cwd: undefined,
        env: {},
        dependsOn: [],
        enabled: true,
      },
    ]);
  };

  const removeService = (index: number) => {
    setServices((prev) => prev.filter((_, i) => i !== index));
  };

  const handleSave = () => {
    if (!detail) return;
    if (activeTab === 'global' && config) {
      saveConfig.mutate(config);
      return;
    }
    if (activeTab === 'project') {
      const nextManifest = buildManifest(detail.manifest, detail.project.name, services);
      setManifest(nextManifest);
      saveManifest.mutate({ projectId: detail.project.id, nextManifest });
      return;
    }
    if (yamlError) {
      toast.error('Fix YAML errors before saving');
      return;
    }
    saveManifest.mutate({ projectId: detail.project.id, nextManifest: manifest });
  };

  return (
    <DashboardLayout>
      <div className="p-8 max-w-[1000px] space-y-6">
        <motion.div
          initial={{ opacity: 0, y: -8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.5, ease: [0.25, 1, 0.5, 1] }}
        >
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Settings</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Project services, global daemon config & manifest editor
          </p>
        </motion.div>

        <motion.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.15, duration: 0.4 }}>
          <div className="flex gap-1.5 rounded-lg bg-muted p-1 mb-6" style={{ width: 'fit-content' }}>
            {([
              { key: 'project', icon: Server, label: 'PROJECT' },
              { key: 'global', icon: Globe, label: 'GLOBAL' },
              { key: 'manifest', icon: FileCode2, label: 'MANIFEST' },
            ] as const).map(tab => (
              <button
                key={tab.key}
                onClick={() => setActiveTab(tab.key)}
                className={cn(
                  'relative inline-flex items-center gap-2 rounded-md px-3 py-2 text-[12px] font-medium transition-colors',
                  activeTab === tab.key ? 'bg-card text-foreground shadow-sm' : 'text-muted-foreground hover:text-foreground',
                )}
              >
                <tab.icon className="h-3.5 w-3.5" />
                {tab.label}
              </button>
            ))}
          </div>
        </motion.div>

        {projects.length > 0 && (
          <div className="flex items-center gap-3">
            <Label className="text-xs uppercase tracking-widest text-muted-foreground">Project</Label>
            <Select value={selectedProject} onValueChange={setSelectedProject}>
              <SelectTrigger className="w-[260px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {projects.map((project) => (
                  <SelectItem key={project.id} value={project.id}>{project.name}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <Button variant="outline" size="sm" onClick={() => selectedProject && api.rescanProject(selectedProject).then(() => queryClient.invalidateQueries())}>
              <RefreshCw className="mr-2 h-3.5 w-3.5" />
              Rescan
            </Button>
          </div>
        )}

        {activeTab === 'project' && detail && (
          <motion.div variants={container} initial="hidden" animate="show" className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-sm font-semibold text-foreground">Service Definitions</h2>
                <p className="text-xs text-muted-foreground">{detail.project.path}</p>
              </div>
              <Button variant="outline" size="sm" onClick={addService}>
                <Plus className="mr-2 h-3.5 w-3.5" />
                Add service
              </Button>
            </div>

            {services.map((service, index) => (
              <motion.div key={service.id} variants={item} className="rounded-lg border border-border bg-card p-5 space-y-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Terminal className="h-4 w-4 text-muted-foreground" />
                    <span className="text-sm font-semibold">{service.name || `Service ${index + 1}`}</span>
                    <Badge variant="secondary">{service.adapter}</Badge>
                  </div>
                  <Button variant="ghost" size="sm" onClick={() => removeService(index)}>
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>

                <div className="grid gap-4 md:grid-cols-2">
                  <div className="space-y-2">
                    <Label>Name</Label>
                    <Input value={service.name} onChange={(e) => handleServiceChange(index, 'name', e.target.value)} />
                  </div>
                  <div className="space-y-2">
                    <Label>Adapter</Label>
                    <Input value={service.adapter} onChange={(e) => handleServiceChange(index, 'adapter', e.target.value)} />
                  </div>
                  <div className="space-y-2 md:col-span-2">
                    <Label>Command</Label>
                    <Input value={service.command} onChange={(e) => handleServiceChange(index, 'command', e.target.value)} />
                  </div>
                  <div className="space-y-2">
                    <Label>Protocol</Label>
                    <Input value={service.protocol} onChange={(e) => handleServiceChange(index, 'protocol', e.target.value)} />
                  </div>
                  <div className="space-y-2">
                    <Label>Route</Label>
                    <Input value={service.route} onChange={(e) => handleServiceChange(index, 'route', e.target.value)} />
                  </div>
                  <div className="space-y-2">
                    <Label>Healthcheck</Label>
                    <Input value={service.healthcheck} onChange={(e) => handleServiceChange(index, 'healthcheck', e.target.value)} />
                  </div>
                  <div className="space-y-2">
                    <Label>Language</Label>
                    <Input value={service.language} onChange={(e) => handleServiceChange(index, 'language', e.target.value)} />
                  </div>
                </div>
              </motion.div>
            ))}
          </motion.div>
        )}

        {activeTab === 'global' && config && (
          <motion.div variants={container} initial="hidden" animate="show" className="grid gap-4 md:grid-cols-2">
            <motion.div variants={item} className="rounded-lg border border-border bg-card p-5 space-y-4">
              <div className="flex items-center gap-2">
                <Globe className="h-4 w-4 text-muted-foreground" />
                <h2 className="text-sm font-semibold">Daemon Config</h2>
              </div>
              <div className="space-y-2">
                <Label>API Port</Label>
                <Input type="number" value={config.apiPort} onChange={(e) => setConfig({ ...config, apiPort: Number(e.target.value) })} />
              </div>
              <div className="space-y-2">
                <Label>Proxy Port</Label>
                <Input type="number" value={config.proxyPort} onChange={(e) => setConfig({ ...config, proxyPort: Number(e.target.value) })} />
              </div>
              <div className="space-y-2">
                <Label>DNS Suffix</Label>
                <Input value={config.dnsSuffix} onChange={(e) => setConfig({ ...config, dnsSuffix: e.target.value })} />
              </div>
              <div className="space-y-2">
                <Label>Log Level</Label>
                <Input value={config.logLevel} onChange={(e) => setConfig({ ...config, logLevel: e.target.value })} />
              </div>
            </motion.div>

            <motion.div variants={item} className="rounded-lg border border-border bg-card p-5 space-y-5">
              <h2 className="text-sm font-semibold">Runtime Switches</h2>
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm">Auto Detect</p>
                  <p className="text-xs text-muted-foreground">Generate manifest draft from project files</p>
                </div>
                <Switch checked={config.autoDetect} onCheckedChange={(checked) => setConfig({ ...config, autoDetect: checked })} />
              </div>
              <Separator />
              <div className="flex items-center justify-between">
                <div>
                  <p className="text-sm">Hot Reload</p>
                  <p className="text-xs text-muted-foreground">Refresh dashboard data when daemon emits updates</p>
                </div>
                <Switch checked={config.hotReload} onCheckedChange={(checked) => setConfig({ ...config, hotReload: checked })} />
              </div>
              <Separator />
              <div className="space-y-2">
                <Label>Healthcheck Interval</Label>
                <Input type="number" value={config.healthcheckInterval} onChange={(e) => setConfig({ ...config, healthcheckInterval: Number(e.target.value) })} />
              </div>
              <div className="space-y-2">
                <Label>Dependency Ready Timeout</Label>
                <Input type="number" value={config.dependencyReadyTimeout} onChange={(e) => setConfig({ ...config, dependencyReadyTimeout: Number(e.target.value) })} />
              </div>
            </motion.div>
          </motion.div>
        )}

        {activeTab === 'manifest' && (
          <motion.div variants={container} initial="hidden" animate="show" className="space-y-4">
            <motion.div variants={item} className="rounded-lg border border-border bg-card overflow-hidden">
              <div className="flex items-center justify-between border-b border-border px-4 py-3">
                <div className="flex items-center gap-2">
                  <FileCode2 className="h-4 w-4 text-muted-foreground" />
                  <span className="text-sm font-semibold">localrouter.yaml</span>
                </div>
                {yamlError ? (
                  <div className="inline-flex items-center gap-1.5 text-xs text-destructive">
                    <AlertCircle className="h-3.5 w-3.5" />
                    {yamlError}
                  </div>
                ) : (
                  <div className="inline-flex items-center gap-1.5 text-xs text-success">
                    <CheckCircle2 className="h-3.5 w-3.5" />
                    Valid YAML
                  </div>
                )}
              </div>
              <Textarea
                value={manifest}
                onChange={(e) => onManifestChange(e.target.value)}
                className="min-h-[420px] rounded-none border-0 bg-transparent font-mono text-[12px]"
              />
            </motion.div>
          </motion.div>
        )}

        <div className="flex justify-end">
          <Button onClick={handleSave} disabled={saveConfig.isPending || saveManifest.isPending}>
            <Save className="mr-2 h-4 w-4" />
            Save changes
          </Button>
        </div>
      </div>
    </DashboardLayout>
  );
}
