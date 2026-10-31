import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import App from './App';
import { applyTheme, readPersistedTheme } from './stores/settingsStore';
import './index.css';

// Apply the last-chosen theme BEFORE React mounts, so the first paint matches
// the user's preference instead of flashing dark and swapping when settings
// load. `useSettingsSync` reconciles it against the saved value after mount.
applyTheme(readPersistedTheme());

window.addEventListener('error', (e) => {
  console.error('[keystone] unhandled error:', e.error ?? e.message);
});
window.addEventListener('unhandledrejection', (e) => {
  console.error('[keystone] unhandled rejection:', e.reason);
});

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      // The library only changes through this app or through a watcher event
      // that invalidates explicitly, so refetching on focus just re-runs a
      // 20,000-row query every time the window is clicked.
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
