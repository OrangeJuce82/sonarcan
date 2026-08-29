import { normalizeImportQuery } from "./importCandidates.ts";
import type { ImportCandidate } from "./types.ts";

type SearchResolver = (query: string) => Promise<ImportCandidate[]>;

export class ImportSearchCache {
  private readonly completed = new Map<string, ImportCandidate[]>();
  private readonly pending = new Map<string, Promise<ImportCandidate[]>>();
  private readonly resolver: SearchResolver;
  private readonly maximumEntries: number;

  constructor(resolver: SearchResolver, maximumEntries = 50) {
    this.resolver = resolver;
    this.maximumEntries = maximumEntries;
  }

  peek(query: string): ImportCandidate[] | undefined {
    const key = normalizeImportQuery(query);
    const cached = this.completed.get(key);
    if (!cached) return undefined;
    this.completed.delete(key);
    this.completed.set(key, cached);
    return cached;
  }

  resolve(query: string): Promise<ImportCandidate[]> {
    const key = normalizeImportQuery(query);
    const cached = this.peek(query);
    if (cached) {
      return Promise.resolve(cached);
    }
    const pending = this.pending.get(key);
    if (pending) return pending;

    const request = this.resolver(query.trim()).then((candidates) => {
      if (this.completed.size >= this.maximumEntries) {
        const oldest = this.completed.keys().next().value;
        if (oldest !== undefined) this.completed.delete(oldest);
      }
      this.completed.set(key, candidates);
      return candidates;
    }).finally(() => this.pending.delete(key));
    this.pending.set(key, request);
    return request;
  }
}
