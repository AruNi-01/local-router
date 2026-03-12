import { toast } from '@/components/ui/sonner';

export async function copyText(value: string, label = 'Value') {
  try {
    await navigator.clipboard.writeText(value);
    toast.success(`${label} copied`);
  } catch {
    toast.error(`Failed to copy ${label.toLowerCase()}`);
  }
}
