import { Component, useState, type ReactNode } from "react";
import { Link, Route, Switch, useLocation } from "wouter";
import {
  Camera as CameraIcon,
  ArrowRight,
  FolderOpen,
  Boxes,
  LayoutDashboard,
  LockKeyhole,
  LogOut,
  PanelLeftClose,
  PanelLeftOpen,
  WifiOff,
  ScanLine,
  SlidersHorizontal,
} from "lucide-react";
import { useAuth } from "./auth";
import { useLiveEvents } from "./events";
import { Dashboard } from "./Dashboard";
import { BatchDetail, Library } from "./Library";
import { Inventory } from "./Inventory";
import { Profiles } from "./Profiles";
import { Scanner } from "./Scanner";
import { Camera } from "./Camera";
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
          name="scanner-login"
          autoComplete="on"
          onSubmit={async (e) => {
            e.preventDefault();
            if (busy) return;
            setBusy(true);
            setError(undefined);
            const credential = key;
            try {
              await connect(credential);
            } catch (error) {
              setError(error);
            } finally {
              setKey("");
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
          <input
            type="hidden"
            name="username"
            autoComplete="username"
            value="scanner"
          />
          <label className="field">
            <span>Access key</span>
            <input
              type="password"
              name="password"
              autoComplete="current-password"
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
  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem("scanner.sidebar.collapsed") === "true";
    } catch {
      return false;
    }
  });
  function toggleSidebar() {
    const next = !collapsed;
    setCollapsed(next);
    try {
      localStorage.setItem("scanner.sidebar.collapsed", String(next));
    } catch {
      /* Still works for this session when browser storage is disabled. */
    }
  }
  const navigation = [
    { href: "/", label: "Dashboard", icon: LayoutDashboard },
    { href: "/camera", label: "Camera", icon: CameraIcon },
    { href: "/library", label: "Library", icon: FolderOpen },
    { href: "/inventory", label: "Inventory", icon: Boxes },
    { href: "/profiles", label: "Profiles", icon: SlidersHorizontal },
    { href: "/scanner", label: "Scanner", icon: ScanLine },
  ];
  return (
    <div className={`workspace${collapsed ? " sidebar-collapsed" : ""}`}>
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
        <div className="sidebar-header">
          <Brand />
          <button
            className="sidebar-toggle"
            onClick={toggleSidebar}
            aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            aria-expanded={!collapsed}
            aria-controls="workspace-navigation"
          >
            {collapsed ? (
              <PanelLeftOpen size={19} />
            ) : (
              <PanelLeftClose size={19} />
            )}
          </button>
        </div>
        <nav id="workspace-navigation" aria-label="Main navigation">
          {navigation.map(({ href, label, icon: Icon }) => (
            <Link
              href={href}
              key={href}
              aria-label={label}
              title={collapsed ? label : undefined}
              aria-current={
                location === href ||
                ((href === "/library" && location.startsWith("/batches/")) || (href === "/inventory" && location.startsWith("/inventory/")))
                  ? "page"
                  : undefined
              }
              className={`nav-link ${location === href || ((href === "/library" && location.startsWith("/batches/")) || (href === "/inventory" && location.startsWith("/inventory/"))) ? "selected" : ""}`}
            >
              <Icon size={19} />
              <span>{label}</span>
            </Link>
          ))}
        </nav>
        <div className="sidebar-bottom">
          {connection === "reconnecting" && (
            <div
              className="connection-warning"
              role="status"
              title="Reconnecting to live updates"
            >
              <WifiOff size={18} aria-hidden="true" />
              <span>Reconnecting</span>
            </div>
          )}
          <button
            className="nav-link logout"
            onClick={logout}
            aria-label="Sign out"
            title={collapsed ? "Sign out" : undefined}
          >
            <LogOut size={18} />
            <span>Sign out</span>
          </button>
        </div>
      </aside>
      <div className="workspace-content">
        <main id="main-content" tabIndex={-1} className="main-content">
          <Switch>
            <Route path="/" component={Dashboard} />
            <Route path="/library" component={Library} />
            <Route path="/inventory"><Inventory /></Route>
            <Route path="/inventory/sets"><Inventory view="sets" /></Route>
            <Route path="/inventory/sets/:setID">{params => <Inventory setID={params.setID} />}</Route>
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
  const [location] = useLocation();
  if (location === "/camera") return <Camera />;
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
