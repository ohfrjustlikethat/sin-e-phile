import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App } from "@/app/App";
import { ErrorBoundary } from "@/app/ErrorBoundary";
import "@/styles/global.css";
import { commands } from "@/lib/ipc";

// TanStack Query owns backend-derived state; Zustand owns client state (§5).
// Defaults are deliberate: this is a desktop app talking to a local backend, so
// network-flavoured retries and refetch-on-focus are noise rather than safety.
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
      staleTime: 30_000,
    },
  },
});

const root = document.getElementById("root");
if (!root) throw new Error("#root is missing from index.html");

// Tell the backend once the first frame has actually reached the screen. Two
// nested rAFs: the first fires before the paint, the second after it. The
// backend reveals the window at that point, so cold start is measured to
// something the user could see, and there is never a flash of empty window.
requestAnimationFrame(() =>
  requestAnimationFrame(() => {
    void commands.frontendReady().catch(() => {
      /* running outside Tauri (vitest, browser) — nothing to reveal */
    });
  }),
);

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <App />
      </QueryClientProvider>
    </ErrorBoundary>
  </React.StrictMode>,
);
