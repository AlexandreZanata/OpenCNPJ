/** Tiny observable store. */
export class Store<S extends object> {
  private readonly listeners = new Set<(state: S) => void>();

  constructor(private state: S) {}

  get(): Readonly<S> {
    return this.state;
  }

  set(patch: Partial<S>): void {
    this.state = { ...this.state, ...patch };
    this.listeners.forEach((listener) => listener(this.state));
  }

  subscribe(listener: (state: S) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}
