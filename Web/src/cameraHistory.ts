import type { CameraSelection } from "./CameraPrice";
export type RecentCard = {
  id: string;
  createdAt: string;
  selection: CameraSelection;
  base64: string;
  quantity: number;
  savedBatchID?: string;
  importStarted?: boolean;
};
const LIMIT = 100;
function database(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open("scanner-camera", 1);
    request.onupgradeneeded = () => request.result.createObjectStore("recent", { keyPath: "id" });
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
async function transaction<T>(mode: IDBTransactionMode, action: (store: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await database();
  try {
    return await new Promise<T>((resolve, reject) => {
      const tx = db.transaction("recent", mode);
      const request = action(tx.objectStore("recent"));
      tx.oncomplete = () => resolve(request.result);
      tx.onerror = () => reject(tx.error);
      tx.onabort = () => reject(tx.error || new Error("Recent scans could not be saved."));
    });
  } finally { db.close(); }
}
export function sameCameraCard(a: CameraSelection, b: CameraSelection) {
  return a.card.id === b.card.id && a.game === b.game && a.language === b.language && a.finish === b.finish;
}
export async function recentCards(): Promise<RecentCard[]> {
  const rows = await transaction<RecentCard[]>("readonly", s => s.getAll());
  return rows.sort((a, b) => b.createdAt.localeCompare(a.createdAt));
}
export async function storeRecentCard(row: RecentCard) {
  await transaction("readwrite", s => s.put(row));
}
export async function removeRecentCard(id: string) { await transaction("readwrite", s => s.delete(id)); }
export async function rememberCard(selection: CameraSelection, base64: string, separate = false): Promise<RecentCard> {
  const rows = await recentCards();
  // Repeated video frames never imply another physical copy. Additional copies
  // are explicit quantities; binder pockets use separate capture IDs.
  const previous = !separate && rows.find(r => sameCameraCard(r.selection, selection));
  if (previous) return previous;
  if (rows.length >= LIMIT) throw new Error("Recent scans are full (100). Remove reviewed entries before capturing more.");
  const row: RecentCard = { id: crypto.randomUUID(), createdAt: new Date().toISOString(), selection, base64, quantity: 1 };
  await storeRecentCard(row);
  return row;
}
