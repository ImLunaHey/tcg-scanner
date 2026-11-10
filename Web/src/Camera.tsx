import { useEffect, useRef, useState } from "react";
import {
  Camera as CameraIcon,
  ScanLine,
  CircleCheck,
  LoaderCircle,
  CircleHelp,
  Square,
  RotateCcw,
  X,
  ChevronRight,
} from "lucide-react";
import { Link } from "wouter";
import { CameraTorch } from "./CameraTorch";
import { CameraFinish } from "./CameraFinish";
import { CameraDetails } from "./CameraDetails";
import { Api, ApiError } from "./api";
import { InstallCamera } from "./InstallCamera";
import {
  difference,
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
  const [phase, setPhase] = useState<
    "opening" | "searching" | "steady" | "identifying" | "detected" | "retry"
  >("searching");
  const [track, setTrack] = useState<MediaStreamTrack>();
  const [error, setError] = useState<string>();
  const [result, setResult] = useState<CameraResult>();
  const [busy, setBusy] = useState(false);
  const [activity, setActivity] = useState("");
  const reset = useRef(() => {});
  useEffect(() => {
    if (!active) return;
    setActivity("");
    let alive = true,
      stream: MediaStream | undefined,
      timer: ReturnType<typeof setTimeout> | undefined;
    const controller = new AbortController();
    const gate = new SteadyCard();
    let inFlight = false,
      started = 0,
      frames = 0,
      attempts = 0;
    let latest: Uint8Array | undefined;
    let generation = 0;
    let lastProgress = 0;
    let lastDuration = "";
    reset.current = () => {
      generation++;
      gate.submitted = undefined;
      gate.matched = false;
      gate.lastAttempt = -Infinity;
      clearMatch();
      setPhase("searching");
      setStatus("Place one card inside the guide");
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
    async function identify(
      captured: HTMLCanvasElement,
      submitted: Uint8Array,
      version: number,
    ) {
      try {
        const base64 = await cameraJPEG(captured);
        const snapshot = await cameraSnapshot(captured);
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
        if (!alive || detailsOpen.current || version !== generation) return;
        if (latest && difference(submitted, latest) >= 12) {
          gate.submitted = undefined;
          gate.lastAttempt = -Infinity;
          setPhase("steady");
          setStatus("View changed · checking the newest frame next");
          return;
        }
        gate.matched = value.state === "matched" || value.state === "codeCard";
        if (!gate.matched)
          gate.lastAttempt = Math.max(gate.lastAttempt, Date.now() - 500);
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
        setPhase(gate.matched ? "detected" : "retry");
        setStatus(
          value.state === "matched"
            ? `${value.candidates[0]?.name || "Card found"} · ready for the next card`
            : value.state === "noCard"
              ? "Keep all four corners inside the guide"
              : value.state === "cardBack"
                ? "Turn the card over to show its front"
                : value.state === "codeCard"
                  ? "Pokémon LIVE code card"
                  : value.state === "ambiguous"
                    ? "Several possible matches · try closer, with less glare"
                    : "No match yet · try closer, with less glare",
        );
      } catch (e) {
        if (!alive || version !== generation || detailsOpen.current) return;
        setError(
          e instanceof ApiError && e.kind === "busy"
            ? "Recognition is busy. Still watching; will retry automatically."
            : cameraMessage(e),
        );
        gate.lastAttempt =
          Date.now() + (e instanceof ApiError && e.status === 429 ? 60_000 : 0);
        setPhase("retry");
        setStatus("Try closer, with more light and less glare");
      } finally {
        inFlight = false;
        lastDuration = `${((Date.now() - started) / 1000).toFixed(1)}s last lookup`;
        if (alive) setBusy(false);
      }
    }
    function tick() {
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
          latest = sample;
          frames++;
          const now = Date.now();
          const ready = gate.ready(sample, now);
          if (now - lastProgress >= 500) {
            lastProgress = now;
            setActivity(
              `${frames} frames checked · ${attempts} lookups${inFlight ? ` · ${((now - started) / 1000).toFixed(1)}s waiting for recognition` : lastDuration ? ` · ${lastDuration}` : ""}`,
            );
          }
          if (inFlight) {
            setPhase("identifying");
            setStatus(
              gate.observation === "moving"
                ? "View moving · still watching while recognition finishes"
                : gate.observation === "searching"
                  ? "Low detail in view · improve lighting or move closer"
                  : now - started > 8000
                    ? "Recognition is taking longer · keep the card clear and avoid glare"
                    : "Reading the captured frame · still watching the live view",
            );
          } else if (ready) {
            gate.accept(sample, Date.now());
            setBusy(true);
            setError(undefined);
            setPhase("identifying");
            setStatus("Sending a clear frame for recognition");
            inFlight = true;
            started = Date.now();
            attempts++;
            const captured = document.createElement("canvas");
            captured.width = canvas.width;
            captured.height = canvas.height;
            captured.getContext("2d")!.drawImage(canvas, 0, 0);
            void identify(captured, sample, generation);
          } else if (gate.observation === "searching") {
            setPhase("searching");
            setStatus("Place one card inside the guide");
          } else if (gate.observation === "moving") {
            setPhase("steady");
            setStatus("Keep the card still with all four corners visible");
          } else if (gate.observation === "unchanged") {
            setPhase("detected");
            setStatus("Card already checked · move to the next card");
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
        setPhase("retry");
        setStatus("Hold steady to retry");
      } finally {
        if (alive) {
          timer = setTimeout(tick, 100);
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
        setPhase("opening");
        setStatus("Allow camera access if prompted");
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
        setTrack(stream.getVideoTracks()[0]);
        const v = video.current!;
        v.srcObject = stream;
        await v.play();
        if (!alive) return;
        setPhase("searching");
        setStatus("Place one card inside the guide");
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
      setTrack(undefined);
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
            <div className="camera-guide" data-phase={phase}>
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
          {active && track && <CameraTorch key={track.id} track={track} />}
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
              <div
                className="camera-status"
                role="status"
                aria-live="polite"
                aria-atomic="true"
                data-phase={phase}
              >
                {phase === "identifying" || phase === "opening" ? (
                  <LoaderCircle className="camera-spinner" size={23} />
                ) : phase === "detected" ? (
                  <CircleCheck size={23} />
                ) : phase === "retry" ? (
                  <CircleHelp size={23} />
                ) : (
                  <ScanLine size={23} />
                )}
                <span>
                  <strong>
                    {
                      {
                        opening: "Opening camera",
                        searching: "Looking for a card",
                        steady: "Hold steady",
                        identifying: "Identifying card…",
                        detected: "Card detected",
                        retry: "Try another view",
                      }[phase]
                    }
                  </strong>
                  <small>{status}</small>
                  <small aria-live="off" data-testid="camera-activity">
                    {activity}
                  </small>
                </span>
              </div>
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
