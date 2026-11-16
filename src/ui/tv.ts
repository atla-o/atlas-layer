import { playShow, searchShows } from "../bridge";

export class TvFinder {
  private timer = 0;

  constructor(
    private readonly box: () => { left: number; top: number; width: number; height: number } | null,
    private readonly onError: (message: string) => void,
  ) {
    const form = document.querySelector<HTMLFormElement>("#tv-finder")!;
    const input = document.querySelector<HTMLInputElement>("#tv-query")!;
    form.addEventListener("submit", (event) => event.preventDefault());
    input.addEventListener("input", () => {
      window.clearTimeout(this.timer);
      const query = input.value;
      this.timer = window.setTimeout(() => {
        void this.look(query);
      }, 220);
    });
    window.addEventListener("resize", () => this.place());
  }

  sync(visible: boolean) {
    const form = document.querySelector<HTMLFormElement>("#tv-finder")!;
    form.hidden = !visible;
    if (!visible) {
      document.querySelector("#tv-results")!.replaceChildren();
      return;
    }
    this.place();
  }

  private place() {
    const form = document.querySelector<HTMLFormElement>("#tv-finder")!;
    if (form.hidden) return;
    const spot = this.box();
    if (!spot) return;
    form.style.left = `${spot.left}px`;
    form.style.top = `${spot.top}px`;
    form.style.width = `${Math.max(72, spot.width)}px`;
  }

  private async look(query: string) {
    const list = document.querySelector<HTMLElement>("#tv-results")!;
    if (query.trim().length < 2) {
      list.replaceChildren();
      return;
    }
    try {
      const shows = await searchShows(query);
      list.replaceChildren();
      for (const show of shows) {
        const item = document.createElement("li");
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = show.year ? `${show.name} ${show.year}` : show.name;
        button.addEventListener("click", () => {
          void playShow(show.kind, show.id).catch((error: unknown) => {
            this.onError(error instanceof Error ? error.message : "Stremio did not open.");
          });
        });
        item.append(button);
        list.append(item);
      }
    } catch (error) {
      this.onError(error instanceof Error ? error.message : "The TV search did not answer.");
    }
  }
}
