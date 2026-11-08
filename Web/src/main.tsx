import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Router } from "wouter";
import { AuthProvider } from "./auth";
import { App, AppBoundary } from "./App";
import { ApiError } from "./api";
import "./styles.css";

// Remove the discarded app-managed image cache; image bytes use HTTP caching.
try { globalThis.indexedDB?.deleteDatabase("scanner-images"); } catch { /* Storage may be disabled. */ }

const queries = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 10_000,
      refetchOnWindowFocus: true,
      retry: (count, error) =>
        count < 1 &&
        !(
          error instanceof ApiError &&
          error.status >= 400 &&
          error.status < 500
        ),
    },
    mutations: { retry: false },
  },
});
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <AppBoundary>
      <QueryClientProvider client={queries}>
        <AuthProvider>
          <Router>
            <App />
          </Router>
        </AuthProvider>
      </QueryClientProvider>
    </AppBoundary>
  </StrictMode>,
);
