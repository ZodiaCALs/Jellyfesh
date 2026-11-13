// Short confirmations ("Plays next"), announced to screen readers through a polite live region.

class Toasts {
  items = $state<{ id: number; text: string }[]>([]);
  private seq = 0;

  show(text: string, ms = 2600) {
    const id = ++this.seq;
    this.items = [...this.items.slice(-2), { id, text }];
    setTimeout(() => (this.items = this.items.filter((i) => i.id !== id)), ms);
  }
}

export const toasts = new Toasts();

export function toast(text: string) {
  toasts.show(text);
}
