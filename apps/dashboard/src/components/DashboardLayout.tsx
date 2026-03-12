import { ReactNode } from 'react';
import { AppSidebar } from './AppSidebar';
import { SidebarProvider, SidebarTrigger } from '@/components/ui/sidebar';
import { TooltipProvider } from '@/components/ui/tooltip';

export function DashboardLayout({ children }: { children: ReactNode }) {
  return (
    <TooltipProvider delayDuration={200}>
      <SidebarProvider>
        <div className="flex h-screen overflow-hidden w-full">
          {/* Grain texture overlay */}
          <div className="grain-overlay" aria-hidden="true" />
          <AppSidebar />
          <div className="flex-1 flex flex-col overflow-hidden">
            <header className="flex h-12 shrink-0 items-center border-b border-border px-4">
              <SidebarTrigger />
            </header>
            <main className="relative flex-1 overflow-y-auto">
              {children}
            </main>
          </div>
        </div>
      </SidebarProvider>
    </TooltipProvider>
  );
}
