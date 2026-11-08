export function inventoryURL({ view = "cards", setID = "", search = "" }: {
  view?: "cards" | "sets"; setID?: string; search?: string;
} = {}): string {
  const path = view === "sets" ? "/inventory/sets" : setID ? `/inventory/sets/${encodeURIComponent(setID)}` : "/inventory";
  const query = new URLSearchParams();
  if (search) query.set("search", search);
  return path + (query.size ? `?${query}` : "");
}
