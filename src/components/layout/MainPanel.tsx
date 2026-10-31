import type { ReactNode } from 'react';

/** The view column: everything right of the sidebar, above the player. */
export function MainPanel({ children }: { children: ReactNode }) {
  return (
    <main className="flex min-w-0 flex-1 flex-col overflow-hidden bg-keystone-black">
      {children}
    </main>
  );
}
