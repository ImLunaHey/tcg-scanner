const batchRoute =
  /^\/batches\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export function legacyRoute(hash: string): string | undefined {
  if (!hash.startsWith("#/")) return;
  const path = hash.slice(1);
  if (
    ["/", "/library", "/profiles", "/scanner"].includes(path) ||
    batchRoute.test(path)
  ) {
    return path;
  }
}

export function migrateHashRoute(): void {
  const path = legacyRoute(window.location.hash);
  if (path) window.history.replaceState(window.history.state, "", path);
}
