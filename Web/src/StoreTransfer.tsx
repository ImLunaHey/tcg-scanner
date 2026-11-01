import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ExternalLink, RefreshCw, Upload } from "lucide-react";
import type { Api } from "./api";
import { ErrorNote } from "./ui";
import type { Failure } from "./models";

export type StoreStatus = {
  configured: boolean;
  state: "notSent" | "sending" | "sent" | "failed";
  sentCards: number;
  totalCards: number;
  preservedCards?: string[];
  reviewURL?: string;
  error?: Failure;
};

export function storeButtonLabel(status?: StoreStatus): string {
  if (status?.state === "sending")
    return `Syncing ${status.sentCards + (status.preservedCards?.length ?? 0)}/${status.totalCards}…`;
  if (status?.state === "sent") return "Resync to Store";
  if (status?.state === "failed") return "Retry sync";
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
            current.state === "sending"
          }
          title="Sync the latest scanner results to Store’s private Scan Inbox. Cards edited or reviewed in Store are preserved."
          onClick={() => send.mutate()}
        >
          {current?.reviewURL ? <RefreshCw size={16} /> : <Upload size={16} />}
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
        {current?.state === "sent" && (
          <span className="small" role="status">
            {current.sentCards} synced
            {!!current.preservedCards?.length && (
              <span title={current.preservedCards.join(", ")}>
                {" · "}
                {current.preservedCards.length} kept unchanged in Store
              </span>
            )}
          </span>
        )}
      </div>
      <ErrorNote
        error={send.error || status.error || current?.error?.message}
      />
    </div>
  );
}
