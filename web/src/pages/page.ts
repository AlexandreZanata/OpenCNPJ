/** A routed page: its element and an optional cleanup run when the route changes. */
export interface Page {
  readonly el: HTMLElement;
  destroy?(): void;
  /** Called when a page kept alive (the assistant) is shown again. */
  resume?(): void;
}
