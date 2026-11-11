/** Per-session, bounded image cache. Clicked full-size images take priority. */
export class ImageRequests {
  private active = 0;
  private pending: { priority: number; run: () => Promise<void> }[] = [];
  private cache = new Map<string, Blob>();
  private bytes = 0;
  constructor(private readonly cacheLimit = 24 * 1024 * 1024) {}

  cached(key: string): Blob | undefined {
    const value = this.cache.get(key);
    if (value) {
      this.cache.delete(key);
      this.cache.set(key, value);
    }
    return value;
  }

  load(
    key: string,
    fetch: () => Promise<Blob>,
    signal?: AbortSignal,
    preview = false,
  ): Promise<Blob> {
    signal?.throwIfAborted();
    const cached = this.cached(key);
    if (cached) {
      this.cache.delete(key);
      this.cache.set(key, cached);
      return Promise.resolve(cached);
    }
    return new Promise((resolve, reject) => {
      this.pending.push({
        priority: preview ? 1 : 0,
        run: async () => {
          try {
            signal?.throwIfAborted();
            const blob = await fetch();
            signal?.throwIfAborted();
            if (blob.size <= this.cacheLimit) {
              const previous = this.cache.get(key);
              if (previous) {
                this.bytes -= previous.size;
                this.cache.delete(key);
              }
              while (this.bytes + blob.size > this.cacheLimit) {
                const oldest = this.cache.keys().next().value!;
                this.bytes -= this.cache.get(oldest)!.size;
                this.cache.delete(oldest);
              }
              this.cache.set(key, blob);
              this.bytes += blob.size;
            }
            resolve(blob);
          } catch (error) {
            reject(error);
          }
        },
      });
      this.pump();
    });
  }
  private pump() {
    this.pending.sort((a, b) => a.priority - b.priority);
    while (this.active < 2 && this.pending.length) {
      const task = this.pending.shift()!;
      this.active++;
      void task.run().finally(() => {
        this.active--;
        this.pump();
      });
    }
  }
}
