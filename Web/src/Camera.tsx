import { useEffect, useRef, useState } from "react";
import {
  Camera as CameraIcon,
  ScanLine,
  Square,
  RotateCcw,
  X,
  ChevronRight,
} from "lucide-react";
import { Link } from "wouter";
import { CameraFinish } from "./CameraFinish";
import { CameraDetails } from "./CameraDetails";
import { Api, ApiError } from "./api";
import { InstallCamera } from "./InstallCamera";
import {
  cameraJPEG,
  cameraSnapshot,
  cameraViewport,
  sampleCard,
  SteadyCard,
} from "./cameraCapture";

type Candidate = {
  id: string;
  name: string;
  setName: string;
  localID: string;
  officialSetCount?: number;
  category?: string;
  types?: string[];
  rarities?: string[];
};
export type CameraResult = {
  state:
    "noCard" | "cardBack" | "codeCard" | "matched" | "ambiguous" | "noMatch";
  game?: "pokemon" | "yugioh" | "starwarsUnlimited" | "unknown";
  candidates: Candidate[];
  uncataloguedCard?: { name: string; setCode: string; collectorNumber: string };
  energySymbol?: { type: string };
  finishEstimate?: import("./Identification").FinishEstimate;
};
const cameraAPI = new Api("");
const gameNames = {
  pokemon: "Pokémon",
  yugioh: "Yu-Gi-Oh!",
  starwarsUnlimited: "Star Wars: Unlimited",
  unknown: "Card",
};
export function cameraMessage(error: unknown) {
  if (error instanceof DOMException) {
    if (error.name === "NotAllowedError")
      return "Camera access was denied. Allow camera access in this site's browser settings, then try again.";
    if (error.name === "NotFoundError")
      return "No camera was found on this device.";
    if (error.name === "NotReadableError")
      return "The camera is in use by another app. Close it and try again.";
  }
  return error instanceof Error
    ? error.message
    : "The camera could not be started.";
}
export function Camera() {
  const frame = useRef<HTMLDivElement>(null);
  const view = useRef<HTMLDivElement>(null);
  const previewURL = useRef<string | undefined>(undefined);
  const [preview, setPreview] = useState<string>();
  const [details, setDetails] = useState(false);
  const detailsOpen = useRef(false);
  function showDetails(open: boolean) {
    detailsOpen.current = open;
    setDetails(open);
  }
  function clearMatch() {
    setResult(undefined);
    setPreview(undefined);
    showDetails(false);
    if (previewURL.current) URL.revokeObjectURL(previewURL.current);
    previewURL.current = undefined;
  }
  useEffect(
    () => () => {
      if (previewURL.current) URL.revokeObjectURL(previewURL.current);
    },
    [],
  );
  const video = useRef<HTMLVideoElement>(null);
  const [active, setActive] = useState(false);
  const [language, setLanguage] = useState("en");
  const [status, setStatus] = useState("Point the camera at a card front");
  const [error, setError] = useState<string>();
  const [result, setResult] = useState<CameraResult>();
  const [busy, setBusy] = useState(false);
  const reset = useRef(() => {});
  useEffect(() => {
    if (!active) return;
    let alive = true,
      stream: MediaStream | undefined,
      timer: ReturnType<typeof setTimeout> | undefined;
    const controller = new AbortController();
    const gate = new SteadyCard();
    reset.current = () => {
      gate.submitted = undefined;
      gate.matched = false;
      gate.lastAttempt = -Infinity;
      clearMatch();
    };
    const canvas = document.createElement("canvas");
    canvas.width = 1000;
    canvas.height = 1333;
    const context = canvas.getContext("2d")!;
    function stopWhenHidden() {
      if (document.hidden || !navigator.onLine) setActive(false);
    }
    document.addEventListener("visibilitychange", stopWhenHidden);
    window.addEventListener("offline", stopWhenHidden);
    async function tick() {
      if (!alive) return;
      try {
        const v = video.current;
        if (
          v &&
          v.readyState >= 2 &&
          v.videoWidth &&
          !document.hidden &&
          !detailsOpen.current &&
          view.current &&
          frame.current
        ) {
          const viewport = view.current.getBoundingClientRect();
          const region = frame.current.getBoundingClientRect();
          const area = cameraViewport(v.videoWidth, v.videoHeight, viewport, {
            x: region.x - viewport.x,
            y: region.y - viewport.y,
            width: region.width,
            height: region.height,
          });
          context.drawImage(
            v,
            area.x,
            area.y,
            area.width,
            area.height,
            0,
            0,
            canvas.width,
            canvas.height,
          );
          const sample = sampleCard(canvas);
          if (gate.ready(sample, Date.now())) {
            gate.accept(sample, Date.now());
            setBusy(true);
            setError(undefined);
            setStatus("Identifying… hold steady");
            const base64 = await cameraJPEG(canvas);
            const snapshot = await cameraSnapshot(canvas);
            if (!alive) return;
            const response = await cameraAPI.response(
              "camera/identify",
              {
                method: "POST",
                signal: controller.signal,
                body: JSON.stringify({ base64, language }),
              },
              false,
              75_000,
            );
            const value: CameraResult = await response.json();
            if (!alive || detailsOpen.current) return;
            gate.matched =
              value.state === "matched" || value.state === "codeCard";
            if (
              value.candidates.length ||
              value.uncataloguedCard ||
              value.energySymbol ||
              value.state === "codeCard"
            ) {
              if (previewURL.current) URL.revokeObjectURL(previewURL.current);
              previewURL.current = snapshot
                ? URL.createObjectURL(snapshot)
                : undefined;
              setPreview(previewURL.current);
              setResult(value);
            }
            setStatus(
              value.state === "matched"
                ? "Card found · move to the next card"
                : value.state === "noCard"
                  ? "Keep all four corners inside the guide"
                  : value.state === "cardBack"
                    ? "Turn the card over to show its front"
                    : value.state === "codeCard"
                      ? "Pokémon LIVE code card"
                      : "Try closer, with less glare",
            );
          }
        }
      } catch (e) {
        if (!alive) return;
        setError(
          e instanceof ApiError && e.kind === "busy"
            ? "The Mac is busy. Identification will retry automatically."
            : cameraMessage(e),
        );
        gate.lastAttempt = Date.now();
        setStatus("Hold steady to retry");
      } finally {
        if (alive) {
          setBusy(false);
          timer = setTimeout(() => void tick(), 200);
        }
      }
    }
    async function start() {
      try {
        if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia)
          throw new Error(
            "Camera access needs HTTPS. Open your scanner's secure Tailscale address.",
          );
        if (!navigator.onLine)
          throw new Error("Connect to the Mac before starting the camera.");
        setError(undefined);
        setStatus("Opening camera…");
        stream = await navigator.mediaDevices.getUserMedia({
          audio: false,
          video: {
            facingMode: { ideal: "environment" },
            width: { ideal: 1920 },
            height: { ideal: 1440 },
          },
        });
        if (!alive) {
          stream.getTracks().forEach((track) => track.stop());
          return;
        }
        const v = video.current!;
        v.srcObject = stream;
        await v.play();
        if (!alive) return;
        setStatus("Fill the guide with one card and hold steady");
        void tick();
      } catch (e) {
        if (alive) {
          setError(cameraMessage(e));
          setActive(false);
        }
      }
    }
    void start();
    return () => {
      alive = false;
      controller.abort();
      clearTimeout(timer);
      stream?.getTracks().forEach((track) => track.stop());
      if (video.current) video.current.srcObject = null;
      document.removeEventListener("visibilitychange", stopWhenHidden);
      window.removeEventListener("offline", stopWhenHidden);
      reset.current = () => {};
      setBusy(false);
    };
  }, [active, language]);

  const title = result?.candidates.length
    ? result.candidates.length === 1
      ? result.candidates[0].name
      : `${result.candidates.length} possible matches`
    : result?.uncataloguedCard?.name ||
      (result?.energySymbol
        ? `${result.energySymbol.type} Energy`
        : "Pokémon LIVE insert");
  const subtitle =
    result?.candidates.length === 1
      ? result.candidates[0].setName
      : result?.game
        ? gameNames[result.game]
        : "View details";
  return (
    <main className="camera-page" aria-label="Card camera">
      <div
        className="camera-surface"
        inert={details}
        aria-hidden={details || undefined}
      >
        <div ref={view} className="camera-view" data-active={active}>
          <video
            ref={video}
            autoPlay
            playsInline
            muted
            aria-label="Live camera preview"
          />
          <div ref={frame} className="camera-capture-area" aria-hidden="true">
            <div className={`camera-guide${busy ? " recognizing" : ""}`}>
              <span />
              <span />
              <span />
              <span />
            </div>
          </div>
          {!active && (
            <div className="camera-placeholder">
              <CameraIcon size={40} />
              <h1>Card camera</h1>
              <p>Hold a card inside the guide to identify it.</p>
              <button
                className="button primary"
                onClick={() => {
                  clearMatch();
                  setActive(true);
                }}
              >
                Start camera
              </button>
              <span>Nothing added to your collection</span>
            </div>
          )}
        </div>
        <div className="camera-toolbar">
          <Link
            href="/"
            className="camera-icon-button"
            aria-label="Close camera"
          >
            <X size={22} />
          </Link>
          <label className="camera-language">
            <span className="sr-only">Card language</span>
            <select
              aria-label="Card language"
              value={language}
              disabled={busy}
              onChange={(e) => {
                clearMatch();
                setLanguage(e.target.value);
              }}
            >
              <option value="en">English</option>
              <option value="ko">Korean</option>
            </select>
          </label>
          {active ? (
            <button
              className="camera-icon-button"
              aria-label="Stop camera"
              onClick={() => setActive(false)}
            >
              <Square size={19} />
            </button>
          ) : (
            <InstallCamera />
          )}
        </div>
        <div className="camera-feedback">
          {error ? (
            <p role="alert" className="camera-error">
              {error}
            </p>
          ) : (
            active && (
              <p className="camera-status" role="status">
                <ScanLine size={17} />
                {status}
              </p>
            )
          )}
        </div>
        {result && (
          <button
            className="camera-match-tile"
            onClick={(event) => {
              event.currentTarget.focus();
              showDetails(true);
            }}
            aria-label={`View ${title} details`}
          >
            {preview && <img src={preview} alt="Last identified card" />}
            <span>
              <small>LAST CARD</small>
              <strong>{title}</strong>
              <span>{subtitle}</span>
              {result.game === "pokemon" && (
                <CameraFinish estimate={result.finishEstimate} compact />
              )}
            </span>
            <ChevronRight size={16} />
          </button>
        )}
        {result && active && (
          <button
            className="camera-icon-button camera-retry"
            aria-label="Check again"
            onClick={() => reset.current()}
          >
            <RotateCcw size={20} />
          </button>
        )}
      </div>
      {details && result && (
        <CameraDetails
          result={result}
          preview={preview}
          title={title}
          game={result.game ? gameNames[result.game] : "IDENTIFIED CARD"}
          language={language}
          close={() => showDetails(false)}
        />
      )}
    </main>
  );
}
