import { useEffect, useRef, useState } from "react";
import {
  Camera as CameraIcon,
  ScanLine,
  Square,
  RotateCcw,
} from "lucide-react";
import { useAuth } from "./auth";
import { ApiError } from "./api";
import { InstallCamera } from "./InstallCamera";
import {
  cameraJPEG,
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
};
export type CameraResult = {
  state:
    "noCard" | "cardBack" | "codeCard" | "matched" | "ambiguous" | "noMatch";
  game?: "pokemon" | "yugioh" | "starwarsUnlimited" | "unknown";
  candidates: Candidate[];
  uncataloguedCard?: { name: string; setCode: string; collectorNumber: string };
  energySymbol?: { type: string };
};
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
  const { api } = useAuth();
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
      setResult(undefined);
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
        if (v && v.readyState >= 2 && v.videoWidth && !document.hidden) {
          const area = cameraViewport(v.videoWidth, v.videoHeight);
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
            setResult(undefined);
            setStatus("Identifying… hold steady");
            const base64 = await cameraJPEG(canvas);
            if (!alive) return;
            const response = await api.response(
              "camera/identify",
              {
                method: "POST",
                signal: controller.signal,
                body: JSON.stringify({ base64, language }),
              },
              true,
              75_000,
            );
            const value: CameraResult = await response.json();
            if (!alive) return;
            gate.matched =
              value.state === "matched" || value.state === "codeCard";
            setResult(value);
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
          timer = setTimeout(() => void tick(), 500);
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
  }, [active, api, language]);
  return (
    <section className="camera-page">
      <header className="camera-heading">
        <div>
          <p className="eyebrow">LOOK UP A CARD</p>
          <h1>Camera</h1>
          <p>Point, hold steady, identify. Nothing added to your collection.</p>
        </div>
        <InstallCamera />
      </header>
      <div className="camera-layout">
        <div className="camera-view" data-active={active}>
          <video
            ref={video}
            autoPlay
            playsInline
            muted
            aria-label="Live camera preview"
          />
          <div
            className={`camera-guide${busy ? " recognizing" : ""}`}
            aria-hidden="true"
          >
            <span />
            <span />
            <span />
            <span />
          </div>
          {!active && (
            <div className="camera-placeholder">
              <CameraIcon size={48} />
              <h2>Find out what you’re holding</h2>
              <p>Show the whole card, with a little space around its edges.</p>
              <button
                className="button primary"
                onClick={() => {
                  setResult(undefined);
                  setActive(true);
                }}
              >
                Start camera
              </button>
            </div>
          )}
          {active && (
            <div className="camera-status" role="status">
              <ScanLine size={18} />
              {status}
            </div>
          )}
        </div>
        <div className="camera-details">
          <div className="camera-controls">
            <label className="field">
              <span>Card language</span>
              <select
                value={language}
                disabled={busy}
                onChange={(e) => {
                  setResult(undefined);
                  setLanguage(e.target.value);
                }}
              >
                <option value="en">English</option>
                <option value="ko">Korean</option>
              </select>
            </label>
            {active && (
              <button
                className="button"
                onClick={() => {
                  setActive(false);
                  setResult(undefined);
                }}
              >
                <Square size={16} />
                Stop camera
              </button>
            )}
          </div>
          {error && (
            <p role="alert" className="camera-error">
              {error}
            </p>
          )}
          <div className="camera-result" aria-live="polite" aria-busy={busy}>
            {busy ? (
              <>
                <span className="eyebrow">IDENTIFYING</span>
                <h2>Reading your card…</h2>
                <p>Keep the number and card name visible.</p>
              </>
            ) : result ? (
              <>
                <span className="eyebrow">
                  {result.game ? gameNames[result.game] : "CAMERA RESULT"}
                </span>
                {result.candidates.length > 0 ? (
                  <>
                    <h2>
                      {result.candidates.length === 1
                        ? result.candidates[0].name
                        : "Possible matches"}
                    </h2>
                    {result.candidates.map((card) => (
                      <div className="camera-match" key={card.id}>
                        {result.candidates.length > 1 && (
                          <strong>{card.name}</strong>
                        )}
                        <span>{card.setName}</span>
                        <span className="muted">
                          {card.localID}
                          {card.officialSetCount
                            ? `/${card.officialSetCount}`
                            : ""}
                        </span>
                      </div>
                    ))}
                    <p className="small muted">
                      Check the printed set and number. Finish and condition
                      aren’t assessed here.
                    </p>
                  </>
                ) : (
                  <>
                    <h2>
                      {result.state === "codeCard"
                        ? "Pokémon LIVE insert"
                        : result.state === "cardBack"
                          ? "Show the card front"
                          : result.uncataloguedCard?.name ||
                            (result.energySymbol
                              ? `${result.energySymbol.type} Energy`
                              : result.state === "noCard"
                                ? "Looking for a card"
                                : "No match yet")}
                    </h2>
                    <p>
                      {result.uncataloguedCard
                        ? `${result.uncataloguedCard.setCode} · ${result.uncataloguedCard.collectorNumber} · not in the catalogue yet`
                        : result.state === "codeCard"
                          ? "This is a redemption insert, not a playable card."
                          : "Keep every edge inside the guide. Avoid glare and use a plain, contrasting background."}
                    </p>
                  </>
                )}
                {active && (
                  <button
                    className="button quiet"
                    onClick={() => reset.current()}
                  >
                    <RotateCcw size={16} />
                    Check again
                  </button>
                )}
              </>
            ) : (
              <>
                <span className="eyebrow">JUST IDENTIFICATION</span>
                <h2>No batch. No inventory changes.</h2>
                <p>
                  Once the card is steady, its image is sent to your Mac for
                  recognition and discarded after the lookup.
                </p>
                <p className="small muted">
                  Your Mac and Tailscale connection need to stay online. Camera
                  access stops when you leave this page or put the app in the
                  background.
                </p>
              </>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
