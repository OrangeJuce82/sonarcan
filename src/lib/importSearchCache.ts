import { normalizeImportQuery } from "./importCandidates.ts";
import type { ImportCandidate } from "./types.ts";

type SearchResolver = (query: string, generation: number) => Promise<ImportCandidate[]>;

interface PendingSearch {
  generation: number;
  request: Promise<ImportCandidate[]>;
}

export class ImportSearchCache {
  private readonly completed = new Map<string, ImportCandidate[]>();
  private readonly pending = new Map<string, PendingSearch>();
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

  resolve(query: string, generation: number): Promise<ImportCandidate[]> {
    const key = normalizeImportQuery(query);
    const cached = this.peek(query);
    if (cached) {
      return Promise.resolve(cached);
    }
    const pending = this.pending.get(key);
    if (pending?.generation === generation) return pending.request;

    const request = this.resolver(query.trim(), generation).then((candidates) => {
      if (this.completed.size >= this.maximumEntries) {
        const oldest = this.completed.keys().next().value;
        if (oldest !== undefined) this.completed.delete(oldest);
      }
      this.completed.set(key, candidates);
      return candidates;
    }).finally(() => {
      if (this.pending.get(key)?.request === request) this.pending.delete(key);
    });
    this.pending.set(key, { generation, request });
    return request;
  }
}
