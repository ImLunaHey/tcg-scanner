export function inventoryURL({ view = "cards", setID = "", search = "", missing = false }: {
  view?: "cards" | "sets"; setID?: string; search?: string; missing?: boolean;
} = {}): string {
  const path = view === "sets" ? "/inventory/sets" : setID ? `/inventory/sets/${encodeURIComponent(setID)}` : "/inventory";
  const query = new URLSearchParams();
  if (search) query.set("search", search);
  if (missing && setID && view === "cards") query.set("missing", "1");
  return path + (query.size ? `?${query}` : "");
}
