import { useQuery } from "@tanstack/react-query";
import { Api } from "./api";
import { CardPrice, type CardValuation } from "./Pricing";
import type { CameraResult } from "./Camera";
export type CameraSelection = {
  card: CameraResult["candidates"][number];
  game: NonNullable<CameraResult["game"]>;
  language: string;
  finish: string;
  finishEstimate?: CameraResult["finishEstimate"];
};
const api = new Api("");
export function selectedCameraCard(result: CameraResult, language: string): CameraSelection | undefined {
  if (result.state !== "matched" || result.candidates.length !== 1 || !result.game || result.game === "unknown") return;
  return { card: result.candidates[0], game: result.game, language, finish: "unverified", finishEstimate: result.finishEstimate };
}
export function CameraPrice({ selection, detail = false }: { selection?: CameraSelection; detail?: boolean }) {
  const query = useQuery({
    queryKey: ["camera-price", selection],
    enabled: !!selection,
    queryFn: async ({ signal }) => {
      const response = await api.response("camera/price", { method: "POST", signal, body: JSON.stringify(selection) }, false);
      return await response.json() as CardValuation;
    },
    staleTime: 60_000,
    retry: 1,
    refetchInterval: q => q.state.data?.state === "pending" ? 3000 : false,
  });
  if (!selection) return null;
  return <div className="camera-price">
    <CardPrice value={query.data} loading={query.isPending} />
    {detail && <small>
      {query.error ? "Price unavailable. Try again shortly." : query.data?.reason}
      {query.data?.lowPence !== undefined && <> Cardmarket trend via TCGdex · GBP.
        {query.data.quotes[0]?.updated && ` Updated ${query.data.quotes[0].updated.slice(0, 10)}.`}
        {query.data.stale && " Cached source data."} Condition is not assessed; this is a price guide.</>}
    </small>}
  </div>;
}
