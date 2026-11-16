import { launchTool, openShelf, searchShelf, stationDetail } from "../bridge";
import type { ShelfHit, StationDetail } from "../types";

const dialog = () => document.querySelector<HTMLDialogElement>("#station-dialog")!;

export class StationPanel {
  private timer = 0;

  constructor() {
    document.querySelector("#station-close")!.addEventListener("click", () => dialog().close());
    document.querySelector<HTMLInputElement>("#station-search")!.addEventListener("keydown", (event) => {
      if (event.key === "Enter") event.preventDefault();
    });
  }

  async open(id: string) {
    const node = dialog();
    const body = document.querySelector<HTMLElement>("#station-body")!;
    const empty = document.querySelector<HTMLElement>("#station-empty")!;
    const error = document.querySelector<HTMLElement>("#station-error")!;
    body.replaceChildren();
    empty.hidden = true;
    error.hidden = true;
    document.querySelector("#station-title")!.textContent = "Opening…";
    const search = document.querySelector<HTMLInputElement>("#station-search")!;
    search.hidden = id !== "shelf";
    search.value = "";
    search.oninput = null;
    if (!node.open) node.showModal();
    if (id === "shelf") {
      document.querySelector("#station-title")!.textContent = "Shelf";
      await this.renderShelf("");
      search.oninput = () => {
        const query = search.value;
        window.clearTimeout(this.timer);
        this.timer = window.setTimeout(() => {
          void this.renderShelf(query);
        }, 180);
      };
      search.focus();
      return;
    }
    try {
      const detail = await stationDetail(id);
      this.render(detail);
    } catch (caught) {
      document.querySelector("#station-title")!.textContent = "Bench";
      error.hidden = false;
      error.textContent = caught instanceof Error ? caught.message : "That did not open.";
    }
  }

  private render(detail: StationDetail) {
    document.querySelector("#station-title")!.textContent = detail.label;
    const body = document.querySelector<HTMLElement>("#station-body")!;
    const empty = document.querySelector<HTMLElement>("#station-empty")!;
    body.replaceChildren();
    empty.hidden = detail.apps.length > 0;
    for (const app of detail.apps) {
      const block = document.createElement("section");
      block.className = "station-app";
      const heading = document.createElement("h3");
      heading.textContent = app.label;
      const open = document.createElement("button");
      open.type = "button";
      open.textContent = `Open ${app.label}`;
      open.addEventListener("click", () => {
        void this.launch(app.id);
      });
      block.append(heading, open);
      const files = detail.files.filter((file) => file.app === app.id);
      if (files.length) {
        const list = document.createElement("div");
        list.className = "station-files";
        for (const file of files) {
          const button = document.createElement("button");
          button.type = "button";
          button.textContent = file.name;
          button.title = file.path;
          button.addEventListener("click", () => {
            void this.launch(app.id, file.path);
          });
          list.append(button);
        }
        block.append(list);
      }
      body.append(block);
    }
  }

  private async renderShelf(query: string) {
    const body = document.querySelector<HTMLElement>("#station-body")!;
    const empty = document.querySelector<HTMLElement>("#station-empty")!;
    const error = document.querySelector<HTMLElement>("#station-error")!;
    error.hidden = true;
    try {
      const hits = await searchShelf(query);
      body.replaceChildren();
      empty.hidden = hits.length > 0;
      empty.textContent = query.trim()
        ? "Nothing matches that."
        : "No launchable apps were found. Type to search files.";
      for (const hit of hits) {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = hit.kind === "file" ? hit.label : hit.label;
        button.title = hit.path;
        button.addEventListener("click", () => {
          void this.openHit(hit);
        });
        body.append(button);
      }
    } catch (caught) {
      error.hidden = false;
      error.textContent = caught instanceof Error ? caught.message : "The shelf did not open.";
    }
  }

  private async openHit(hit: ShelfHit) {
    const error = document.querySelector<HTMLElement>("#station-error")!;
    error.hidden = true;
    try {
      await openShelf(hit);
    } catch (caught) {
      error.hidden = false;
      error.textContent = caught instanceof Error ? caught.message : "That did not open.";
    }
  }

  private async launch(appId: string, file?: string) {
    const error = document.querySelector<HTMLElement>("#station-error")!;
    error.hidden = true;
    try {
      await launchTool(appId, file);
    } catch (caught) {
      error.hidden = false;
      error.textContent = caught instanceof Error ? caught.message : "That did not launch.";
    }
  }
}
