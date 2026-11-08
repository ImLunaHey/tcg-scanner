const databaseName = "scanner-images";
const maximumBytes = 256 * 1024 * 1024;
const maximumImageBytes = 32 * 1024 * 1024;
type Metadata = { key: string; bytes: number; type: string; used: number; expires: number };

/** A bounded browser disk cache, independent of bundles and rotating Bearer tokens.
 * Only authenticated API instances supply a server-issued scope. No credentials
 * are stored here. IndexedDB failure is optional: the caller still uses HTTP.
 */
export class PersistentImageCache {
  private opening?: Promise<IDBDatabase | undefined>;
  constructor(private readonly name = databaseName, private readonly limit = maximumBytes) {}
  private open(): Promise<IDBDatabase | undefined> {
    if (this.opening) return this.opening;
    this.opening = new Promise<IDBDatabase | undefined>(resolve => {
      if (typeof indexedDB === "undefined") { resolve(undefined); return; }
      let done = false;
      const finish = (db?: IDBDatabase) => { if (!done) { done = true; resolve(db); } else db?.close(); };
      const request = indexedDB.open(this.name, 1);
      request.onupgradeneeded = () => {
        request.result.createObjectStore("images");
        request.result.createObjectStore("metadata", { keyPath: "key" });
      };
      request.onerror = () => finish();
      request.onblocked = () => finish();
      request.onsuccess = () => {
        const db = request.result;
        db.onversionchange = () => { db.close(); this.opening = undefined; };
        finish(db);
      };
    }).catch(() => undefined);
    return this.opening;
  }
  async get(key: string): Promise<Blob | undefined> {
    try {
      const db = await this.open();
      if (!db) return;
      return await new Promise<Blob | undefined>((resolve, reject) => {
        const transaction = db.transaction(["images", "metadata"], "readwrite");
        const metadata = transaction.objectStore("metadata"), images = transaction.objectStore("images");
        let blob: Blob | undefined;
        transaction.oncomplete = () => resolve(blob);
        transaction.onerror = transaction.onabort = () => reject(transaction.error);
        const record = metadata.get(key);
        record.onsuccess = () => {
          const value = record.result as Metadata | undefined;
          if (!value || value.expires <= Date.now()) { metadata.delete(key); images.delete(key); return; }
          const image = images.get(key);
          image.onsuccess = () => {
            if (image.result instanceof ArrayBuffer && image.result.byteLength === value.bytes) {
              blob = new Blob([image.result], { type: value.type });
              metadata.put({ ...value, used: Date.now() });
            } else { metadata.delete(key); images.delete(key); }
          };
        };
      });
    } catch { return undefined; }
  }
  async put(key: string, blob: Blob, lifetime: number): Promise<void> {
    if (!blob.size || blob.size > Math.min(this.limit, maximumImageBytes) || !/^image\/(png|jpeg|tiff|webp)$/.test(blob.type)) return;
    try {
      const db = await this.open();
      if (!db) return;
      // Store bytes consistently across engines; WebKit can reject IDB Blob writes.
      const bytes = await blob.arrayBuffer();
      await new Promise<void>((resolve, reject) => {
        const transaction = db.transaction(["images", "metadata"], "readwrite");
        const metadata = transaction.objectStore("metadata"), images = transaction.objectStore("images");
        transaction.oncomplete = () => resolve();
        transaction.onerror = transaction.onabort = () => reject(transaction.error);
        const records = metadata.getAll();
        records.onsuccess = () => {
          const now = Date.now(), keep: Metadata[] = [];
          for (const entry of records.result as Metadata[]) {
            if (entry.key === key || entry.expires <= now) { metadata.delete(entry.key); images.delete(entry.key); }
            else keep.push(entry);
          }
          let used = keep.reduce((sum, entry) => sum + entry.bytes, 0);
          for (const entry of keep.sort((a, b) => a.used - b.used)) {
            if (used + blob.size <= this.limit) break;
            metadata.delete(entry.key); images.delete(entry.key); used -= entry.bytes;
          }
          images.put(bytes, key);
          metadata.put({ key, bytes: blob.size, type: blob.type, used: now, expires: now + lifetime } satisfies Metadata);
        };
      });
    } catch { /* Quota, eviction and private-mode failures must not prevent display. */ }
  }
}

export function persistentImageKey(scope: string | undefined, path: string, sourceSHA256: string): string | undefined {
  if (!scope || !/^[a-f0-9]{64}$/.test(scope)) return;
  // Deliberately independent of application builds. Bump only if image rendering
  // changes without changing its source hash or immutable edit ID.
  return JSON.stringify(["image-v1", scope, path, sourceSHA256]);
}
export const persistentImages = new PersistentImageCache();
