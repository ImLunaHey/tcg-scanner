export function newestSetsFirst<T extends { id: string; name: string; releaseDate?: string }>(sets: T[]): T[] {
  return [...sets].sort((a, b) => (b.releaseDate || "").localeCompare(a.releaseDate || "")
    || a.name.localeCompare(b.name) || a.id.localeCompare(b.id));
}
export function setReleaseLabel(date?: string): string {
  return date ? new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" }).format(new Date(date)) : "Release date unknown";
}
