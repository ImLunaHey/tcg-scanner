import { Component, useState, type ReactNode } from "react";
import { Link, Route, Switch, useLocation } from "wouter";
import {
  ArrowRight,
  FolderOpen,
  LayoutDashboard,
  LockKeyhole,
  LogOut,
  ScanLine,
  SlidersHorizontal,
} from "lucide-react";
import { useAuth } from "./auth";
import { useLiveEvents } from "./events";
import { Dashboard } from "./Dashboard";
import { BatchDetail, Library } from "./Library";
import { Profiles } from "./Profiles";
import { Scanner } from "./Scanner";
import { Brand, ErrorNote } from "./ui";

function Login() {
  const { connect } = useAuth();
  const [key, setKey] = useState("");
  const [error, setError] = useState<unknown>();
  const [busy, setBusy] = useState(false);
  return (
    <div className="login-layout">
      <section className="login-story">
        <Brand />
        <div>
          <p className="eyebrow">FROM FEEDER TO FOLDER</p>
          <h1>
            Every scan.
            <br />
            In its place.
          </h1>
          <p>
            Your scanner, wherever you work.
            <br />
            One shared library. Originals always preserved.
          </p>
          <div className="scan-art" aria-hidden="true">
            <div className="art-card back" />
            <div className="art-card front">
              <ScanLine size={56} />
              <span>B-001 / C-001</span>
            </div>
            <div className="art-line" />
          </div>
        </div>
        <span className="small">NATIVE SCANNING · REMOTE CONTROL</span>
      </section>
      <main className="login-form-area">
        <form
          className="login-form"
          onSubmit={async (e) => {
            e.preventDefault();
            if (busy) return;
            setBusy(true);
            setError(undefined);
            const credential = key;
            setKey("");
            try {
              await connect(credential);
            } catch (error) {
              setError(error);
            } finally {
              setBusy(false);
            }
          }}
        >
          <span className="login-lock">
            <LockKeyhole size={24} />
          </span>
          <p className="eyebrow">PRIVATE WORKSPACE</p>
          <h2>Connect to your scanner</h2>
          <p>
            Enter your access key to open this workspace. Your session stays in
            this browser tab for up to one hour.
          </p>
          <label className="field">
            <span>Access key</span>
            <input
              type="password"
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              required
              value={key}
              onChange={(e) => setKey(e.target.value)}
              placeholder="Paste your scanner access key"
            />
          </label>
          <ErrorNote error={error} />
          <button
            className="button primary full"
            disabled={busy || !key.trim()}
          >
            {busy ? "Connecting…" : "Open workspace"}
            <ArrowRight size={18} />
          </button>
          <details className="login-help">
            <summary>Where do I find my access key?</summary>
            <p>
              On the Mac running the scanner service, copy the key from the
              private gateway token file. The installation guide includes the
              clipboard command. Keep the key private; it grants scanner
              control.
            </p>
          </details>
          <p className="small muted login-footnote">
            Access is restricted to your configured connection. Signing in never
            starts the scanner.
          </p>
        </form>
      </main>
    </div>
  );
}
function Workspace() {
  const { logout } = useAuth();
  const connection = useLiveEvents();
  const [location] = useLocation();
  const navigation = [
    { href: "/", label: "Dashboard", icon: LayoutDashboard },
    { href: "/library", label: "Library", icon: FolderOpen },
    { href: "/profiles", label: "Profiles", icon: SlidersHorizontal },
    { href: "/scanner", label: "Scanner", icon: ScanLine },
  ];
  return (
    <div className="workspace">
      <a
        className="skip-link"
        href="#main-content"
        onClick={(event) => {
          event.preventDefault();
          document.getElementById("main-content")?.focus();
        }}
      >
        Skip to content
      </a>
      <aside className="sidebar">
        <Brand />
        <div className="nav-caption">WORKSPACE</div>
        <nav aria-label="Main navigation">
          {navigation.map(({ href, label, icon: Icon }) => (
            <Link
              href={href}
              key={href}
              className={`nav-link ${location === href || (href === "/library" && location.startsWith("/batches/")) ? "selected" : ""}`}
            >
              <Icon size={19} />
              <span>{label}</span>
            </Link>
          ))}
        </nav>
        <div className="sidebar-bottom">
          <div className="sidebar-note">
            <span className="small">MADE FOR YOUR WORKFLOW</span>
            <p>
              Capture once.
              <br />
              Keep the original.
            </p>
          </div>
          <button className="nav-link logout" onClick={logout}>
            <LogOut size={18} />
            <span>Sign out</span>
          </button>
        </div>
      </aside>
      <div className="workspace-content">
        <div className="topbar">
          <span className="breadcrumb">
            Scanner <span>/</span>{" "}
            {location.startsWith("/batches/")
              ? "Batch details"
              : navigation.find((item) => item.href === location)?.label ||
                "Workspace"}
          </span>
          <div className={`live-status ${connection}`} role="status">
            <span className="status-dot" />
            {connection === "live"
              ? "Live updates"
              : connection === "connecting"
                ? "Connecting"
                : "Reconnecting"}
          </div>
        </div>
        <main id="main-content" tabIndex={-1} className="main-content">
          <Switch>
            <Route path="/" component={Dashboard} />
            <Route path="/library" component={Library} />
            <Route path="/profiles" component={Profiles} />
            <Route path="/scanner" component={Scanner} />
            <Route path="/batches/:id">
              {(params) => <BatchDetail key={params.id} id={params.id} />}
            </Route>
            <Route>
              <h1>Page not found</h1>
              <Link href="/">Back to dashboard</Link>
            </Route>
          </Switch>
        </main>
        <footer className="workspace-footer">
          <span>Scanner workspace</span>
          <span>Originals preserved · One shared service</span>
        </footer>
      </div>
    </div>
  );
}
export function App() {
  const { session } = useAuth();
  return session ? <Workspace /> : <Login />;
}
export class AppBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? (
      <main className="empty">
        <h1>The workspace couldn’t be displayed.</h1>
        <p>
          Reload to reconnect. Existing scanner jobs continue in the background.
        </p>
        <button
          className="button primary"
          onClick={() => window.location.reload()}
        >
          Reload workspace
        </button>
      </main>
    ) : (
      this.props.children
    );
  }
}
