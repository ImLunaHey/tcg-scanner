import { useEffect, useRef, useState } from "react";
import { Zap, ZapOff } from "lucide-react";

// Torch is part of Image Capture; not every browser includes it in DOM types.
type TorchSettings = MediaTrackSettings & { torch?: boolean };
type TorchCapabilities = MediaTrackCapabilities & { torch?: boolean };
type TorchConstraint = MediaTrackConstraintSet & { torch: boolean };

export function CameraTorch({ track }: { track: MediaStreamTrack }) {
  const alive = useRef(true);
  const [supported] = useState(() => {
    try {
      return (track.getCapabilities?.() as TorchCapabilities)?.torch === true;
    } catch {
      return false;
    }
  });
  const [on, setOn] = useState(
    () => (track.getSettings?.() as TorchSettings)?.torch === true,
  );
  const [changing, setChanging] = useState(false);
  const [error, setError] = useState<string>();
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  async function toggle() {
    if (!supported || changing) return;
    setChanging(true);
    setError(undefined);
    try {
      const desired = !on;
      const constraint: TorchConstraint = { torch: desired };
      await track.applyConstraints({
        ...track.getConstraints(),
        advanced: [constraint],
      });
      if (!alive.current || track.readyState === "ended") return;
      const actual = (track.getSettings() as TorchSettings).torch;
      // A resolved constraint request alone does not prove the light changed.
      if (actual !== desired) throw new Error("Torch setting was not applied");
      setOn(actual);
    } catch {
      if (alive.current)
        setError("Couldn’t change the flash. Try again or use another light.");
    } finally {
      if (alive.current) setChanging(false);
    }
  }
  return (
    <div className="camera-torch">
      <button
        className="camera-icon-button"
        disabled={!supported || changing}
        aria-label={supported ? "Flash" : "Flash unavailable on this camera"}
        aria-pressed={on}
        title={
          supported
            ? `Turn flash ${on ? "off" : "on"}`
            : "This camera or browser does not support flash"
        }
        onClick={() => void toggle()}
      >
        {supported ? <Zap size={21} /> : <ZapOff size={21} />}
        <span>{supported ? (on ? "On" : "Off") : "N/A"}</span>
      </button>
      {error && (
        <p role="alert" className="camera-torch-error">
          {error}
        </p>
      )}
    </div>
  );
}
