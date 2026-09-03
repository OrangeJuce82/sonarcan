export interface BackgroundTask {
  scope: string;
  key: string;
  run: () => Promise<void>;
}

export interface BackgroundTaskSchedulerOptions {
  idleDelayMs?: number;
  betweenTasksMs?: number;
}

export class BackgroundTaskScheduler {
  private readonly idleDelayMs: number;
  private readonly betweenTasksMs: number;
  private readonly queue: BackgroundTask[] = [];
  private blocked = true;
  private running = false;
  private current: BackgroundTask | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;

  constructor(options: BackgroundTaskSchedulerOptions = {}) {
    this.idleDelayMs = options.idleDelayMs ?? 1_000;
    this.betweenTasksMs = options.betweenTasksMs ?? 250;
  }

  enqueue(task: BackgroundTask): void {
    if (
      (this.current?.scope === task.scope && this.current.key === task.key)
      || this.queue.some((queued) => queued.scope === task.scope && queued.key === task.key)
    ) return;
    this.queue.push(task);
    this.schedule(this.idleDelayMs);
  }

  setBlocked(blocked: boolean): void {
    if (this.blocked === blocked) return;
    this.blocked = blocked;
    if (blocked) this.clearTimer();
    else this.schedule(this.idleDelayMs);
  }

  cancelScope(scope: string): void {
    for (let index = this.queue.length - 1; index >= 0; index -= 1) {
      if (this.queue[index]?.scope === scope) this.queue.splice(index, 1);
    }
    if (this.queue.length === 0) this.clearTimer();
  }

  cancelAll(): void {
    this.queue.length = 0;
    this.clearTimer();
  }

  private schedule(delayMs: number): void {
    if (this.blocked || this.running || this.timer !== undefined || this.queue.length === 0) return;
    this.timer = setTimeout(() => {
      this.timer = undefined;
      void this.runNext();
    }, delayMs);
  }

  private async runNext(): Promise<void> {
    if (this.blocked || this.running) return;
    const task = this.queue.shift();
    if (!task) return;
    this.running = true;
    this.current = task;
    try {
      await task.run();
    } catch {
      // Background warming is best-effort; foreground work must remain unaffected.
    } finally {
      this.running = false;
      this.current = undefined;
      this.schedule(this.betweenTasksMs);
    }
  }

  private clearTimer(): void {
    if (this.timer === undefined) return;
    clearTimeout(this.timer);
    this.timer = undefined;
  }
}
