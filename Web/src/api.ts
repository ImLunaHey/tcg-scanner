import { ImageRequests } from "./ImageRequests";

export class ApiError extends Error {
  constructor(
    message: string,
    public status = 0,
    public kind = "network",
  ) {
    super(message);
  }
}
export type Session = { token: string; expiresAt: string };
const sessionKey = "scanner.session.v1";
export function loadSession(): Session | null {
  try {
    const value = JSON.parse(sessionStorage.getItem(sessionKey) || "null");
    return value &&
      /^[a-f0-9]{64}$/.test(value.token) &&
      Date.parse(value.expiresAt) > Date.now()
      ? value
      : null;
  } catch {
    return null;
  }
}
export function storeSession(value: Session | null) {
  try {
    if (value) sessionStorage.setItem(sessionKey, JSON.stringify(value));
    else sessionStorage.removeItem(sessionKey);
  } catch {
    /* Private browsing can disallow storage; memory still works. */
  }
}
export async function responseError(response: Response): Promise<ApiError> {
  const body = await response.json().catch(() => ({}));
  return new ApiError(
    body.error?.message || `Request failed (${response.status}).`,
    response.status,
    body.error?.kind || "http",
  );
}
export class Api {
  constructor(
    private token: string,
    private expired: () => void = () => {},
  ) {}
  async response(path: string, options: RequestInit = {}): Promise<Response> {
    const response = await fetch(`/api/v1/${path}`, {
      ...options,
      credentials: "omit",
      cache: "no-store",
      headers: {
        Authorization: `Bearer ${this.token}`,
        ...(options.body ? { "Content-Type": "application/json" } : {}),
        ...options.headers,
      },
      signal: options.signal
        ? AbortSignal.any([options.signal, AbortSignal.timeout(30_000)])
        : AbortSignal.timeout(30_000),
    }).catch((error) => {
      if (error.name === "AbortError") throw error;
      throw new ApiError(
        "Cannot reach the scanner service. Check your connection and retry.",
      );
    });
    if (!response.ok) {
      if (response.status === 401) this.expired();
      throw await responseError(response);
    }
    return response;
  }
  async get<T>(path: string, signal?: AbortSignal): Promise<T> {
    return (await this.response(path, { signal })).json();
  }
  async send<T>(
    path: string,
    method: "POST" | "PATCH" | "DELETE",
    data?: unknown,
  ): Promise<T> {
    return (
      await this.response(path, {
        method,
        body: data === undefined ? undefined : JSON.stringify(data),
      })
    ).json();
  }
  async blob(path: string, signal?: AbortSignal) {
    return (await this.response(path, { signal })).blob();
  }
}
export function saveBlob(blob: Blob, name: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 30_000);
}
export function saveJSON(value: unknown, name: string) {
  saveBlob(
    new Blob([JSON.stringify(value, null, 2)], { type: "application/json" }),
    name,
  );
}

// Weakly scoped to the authenticated API instance; signing out drops its cache.
const imageRequests = new WeakMap<Api, ImageRequests>();
export function imageBlob(
  api: Api,
  path: string,
  signal?: AbortSignal,
  sourceSHA256 = "",
): Promise<Blob> {
  let loader = imageRequests.get(api);
  if (!loader) {
    loader = new ImageRequests();
    imageRequests.set(api, loader);
  }
  return loader.load(
    path + ":" + sourceSHA256,
    async () => {
      for (let attempt = 0; ; attempt++) {
        signal?.throwIfAborted();
        try {
          return await api.blob(path, signal);
        } catch (error) {
          if (
            !(error instanceof ApiError) ||
            error.status !== 429 ||
            attempt >= 6
          )
            throw error;
          await new Promise((resolve) =>
            setTimeout(
              resolve,
              500 * 2 ** Math.min(attempt, 3) + Math.random() * 250,
            ),
          );
        }
      }
    },
    signal,
    path.includes("/thumbnail"),
  );
}
