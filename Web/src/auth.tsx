import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useQueryClient } from "@tanstack/react-query";
import { Api, loadSession, storeSession, type Session } from "./api";

const AuthContext = createContext<{
  api: Api;
  session: Session | null;
  connect: (key: string) => Promise<void>;
  logout: () => void;
} | null>(null);
export function AuthProvider({ children }: { children: ReactNode }) {
  const [session, setSession] = useState(loadSession);
  const queries = useQueryClient();
  const clear = useCallback(() => {
    storeSession(null);
    setSession(null);
    queries.clear();
  }, [queries]);
  const api = useMemo(
    () => new Api(session?.token || "", clear),
    [session, clear],
  );
  async function connect(key: string) {
    const value = await new Api(key.trim()).send<Session>("session", "POST");
    storeSession(value);
    setSession(value);
  }
  function logout() {
    void api.send("session", "DELETE").catch(() => {});
    clear();
  }
  return (
    <AuthContext.Provider value={{ api, session, connect, logout }}>
      {children}
    </AuthContext.Provider>
  );
}
export function useAuth() {
  const value = useContext(AuthContext);
  if (!value) throw new Error("Missing authentication context");
  return value;
}
