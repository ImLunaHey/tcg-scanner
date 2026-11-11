import { useEffect, useState } from "react";
import { Download } from "lucide-react";
type InstallPrompt = Event & {
  prompt(): Promise<void>;
  userChoice: Promise<{ outcome: string }>;
};
export function InstallCamera() {
  const [prompt, setPrompt] = useState<InstallPrompt>();
  const [installed, setInstalled] = useState(
    () => window.matchMedia("(display-mode: standalone)").matches,
  );
  const [help, setHelp] = useState(false);
  useEffect(() => {
    const available = (e: Event) => {
      e.preventDefault();
      setPrompt(e as InstallPrompt);
    };
    const done = () => {
      setInstalled(true);
      setPrompt(undefined);
    };
    window.addEventListener("beforeinstallprompt", available);
    window.addEventListener("appinstalled", done);
    return () => {
      window.removeEventListener("beforeinstallprompt", available);
      window.removeEventListener("appinstalled", done);
    };
  }, []);
  if (installed) return null;
  return (
    <div className="camera-install">
      <button
        className="button"
        onClick={async () => {
          if (!prompt) {
            setHelp(!help);
            return;
          }
          try {
            await prompt.prompt();
            await prompt.userChoice;
          } catch {
            setHelp(true);
          } finally {
            setPrompt(undefined);
          }
        }}
      >
        <Download size={17} />
        Install app
      </button>
      {help && (
        <p className="small" role="status">
          On iPhone/iPad, open in Safari, tap Share, then Add to Home Screen. On
          Android, use your browser’s Install app or Add to Home screen menu.
        </p>
      )}
    </div>
  );
}
