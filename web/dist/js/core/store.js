/** Tiny observable store. */
export class Store {
    state;
    listeners = new Set();
    constructor(state) {
        this.state = state;
    }
    get() {
        return this.state;
    }
    set(patch) {
        this.state = { ...this.state, ...patch };
        this.listeners.forEach((listener) => listener(this.state));
    }
    subscribe(listener) {
        this.listeners.add(listener);
        return () => this.listeners.delete(listener);
    }
}
//# sourceMappingURL=store.js.map