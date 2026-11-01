import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, Upload } from "lucide-react";
import type { Api } from "./api";
import { ErrorNote } from "./ui";
import type { Failure } from "./models";

export type StoreStatus = {
  configured: boolean;
  state: "notSent" | "sending" | "sent" | "failed";
  sentCards: number;
  totalCards: number;
  reviewURL?: string;
  error?: Failure;
};

export function storeButtonLabel(status?: StoreStatus): string {
  if (status?.state === "sending")
    return `Sending ${status.sentCards}/${status.totalCards}…`;
  if (status?.state === "sent") return "Sent for review";
  if (status?.state === "failed") return "Retry sending";
  return "Send to Store";
}

export function StoreTransfer({ api, id }: { api: Api; id: string }) {
  const cache = useQueryClient();
  const queryKey = ["store-transfer", id];
  const path = `batches/${encodeURIComponent(id)}/store`;
  const status = useQuery({
    queryKey,
    queryFn: ({ signal }) => api.get<StoreStatus>(path, signal),
    refetchInterval: (query) =>
      query.state.data?.state === "sending" ? 1000 : 15000,
  });
  const send = useMutation({
    mutationFn: () => api.send<StoreStatus>(path, "POST"),
    onSuccess: (value) => cache.setQueryData(queryKey, value),
    onError: () => {
      void cache.invalidateQueries({ queryKey });
    },
  });
  const current = status.data;
  return (
    <div className="store-transfer" aria-label="Store transfer">
      <div className="store-transfer-actions">
        <button
          className="button secondary"
          disabled={
            !current?.configured ||
            send.isPending ||
            current.state === "sending" ||
            current.state === "sent"
          }
          title="Send this batch to Store’s private Scan Inbox for review."
          onClick={() => send.mutate()}
        >
          <Upload size={16} />
          <span aria-live="polite">
            {send.isPending ? "Starting transfer…" : storeButtonLabel(current)}
          </span>
        </button>
        {current?.reviewURL && (
          <a
            className="button secondary"
            href={current.reviewURL}
            target="_blank"
            rel="noopener noreferrer"
          >
            <ExternalLink size={16} /> Review in Store
          </a>
        )}
        {current && !current.configured && (
          <span className="small">Store connection unavailable</span>
        )}
      </div>
      <ErrorNote
        error={send.error || status.error || current?.error?.message}
      />
    </div>
  );
}
