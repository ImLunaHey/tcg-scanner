import { useEffect, useState } from "react";
import { useQueryClient, type QueryClient } from "@tanstack/react-query";
import { useAuth } from "./auth";
import { ApiError } from "./api";

export type SSEMessage = { event: string; data: string; id?: string };
export function parseSSE(frame: string): SSEMessage {
  const result: SSEMessage = { event: "message", data: "" };
  for (const line of frame.split("\n")) {
    if (line.startsWith("event:")) result.event = line.slice(6).trimStart();
    if (line.startsWith("id:")) result.id = line.slice(3).trim();
    if (line.startsWith("data:"))
      result.data += (result.data ? "\n" : "") + line.slice(5).trimStart();
  }
  return result;
}
const liveQueryKeys = ["inventory", "status", "jobs", "batch", "devices", "profiles", "store-transfer"];
export function eventQueryKeys(event: SSEMessage): string[] {
  if (event.event === "ready" || event.event === "reset") return liveQueryKeys;
  if (event.event !== "scanner") return [];
  try {
    const value = JSON.parse(event.data);
    // Inventory includes terminal batches only. Button diagnostics and capture
    // progress do not change stock, so must not restart inventory pagination.
    const inventoryChanged = value.kind === "job.updated" &&
      ["completed", "failed", "cancelled"].includes(value.payload?.state);
    return liveQueryKeys.filter(key => key !== "inventory" || inventoryChanged);
  } catch { return []; }
}
export function refreshEventQueries(query: QueryClient, keys: ReadonlySet<string>) {
  return query.invalidateQueries({ predicate: q => keys.has(String(q.queryKey[0])) }, { cancelRefetch: false });
}
export function useLiveEvents() {
  const { api } = useAuth();
  const query = useQueryClient();
  const [connection, setConnection] = useState<
    "connecting" | "live" | "reconnecting"
  >("connecting");
  useEffect(() => {
    const controller = new AbortController();
    let cursor: string | undefined;
    let refresh: ReturnType<typeof setTimeout> | undefined;
    const pending = new Set<string>();
    const invalidate = (event: SSEMessage) => {
      for (const key of eventQueryKeys(event)) pending.add(key);
      if (!refresh && pending.size)
        refresh = setTimeout(() => {
          refresh = undefined;
          const keys = new Set(pending); pending.clear();
          void refreshEventQueries(query, keys);
        }, 300);
    };
    async function run() {
      while (!controller.signal.aborted) {
        try {
          const response = await api.response(
            `events?seconds=25${cursor === undefined ? "" : `&after=${cursor}`}`,
            { signal: controller.signal },
          );
          if (
            !response.headers.get("Content-Type")?.includes("text/event-stream")
          )
            throw new Error("Unexpected event response");
          if (!response.body) throw new Error("Missing event stream");
          setConnection("live");
          const reader = response.body.getReader();
          const decoder = new TextDecoder();
          let buffer = "";
          try {
            while (true) {
              const chunk = await reader.read();
              if (chunk.done) break;
              buffer = (
                buffer + decoder.decode(chunk.value, { stream: true })
              ).replace(/\r\n/g, "\n");
              if (buffer.length > 1_000_000)
                throw new Error("Event exceeds client limit");
              let end: number;
              while ((end = buffer.indexOf("\n\n")) >= 0) {
                const event = parseSSE(buffer.slice(0, end));
                buffer = buffer.slice(end + 2);
                if (event.id && /^\d+$/.test(event.id)) cursor = event.id;
                invalidate(event);
                if (event.event === "error")
                  throw new Error("Stream interrupted");
              }
            }
          } finally {
            await reader.cancel().catch(() => {});
            reader.releaseLock();
          }
        } catch (error) {
          if (controller.signal.aborted) break;
          setConnection("reconnecting");
          // Polling continues independently while SSE reconnects.
          await new Promise<void>((resolve) => {
            const done = () => {
              clearTimeout(timer);
              controller.signal.removeEventListener("abort", done);
              resolve();
            };
            const timer = setTimeout(done, error instanceof ApiError && error.status === 429 ? 60_000 : 3000);
            controller.signal.addEventListener("abort", done, { once: true });
          });
        }
      }
    }
    void run();
    return () => {
      controller.abort();
      clearTimeout(refresh);
    };
  }, [api, query]);
  return connection;
}
