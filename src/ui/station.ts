import { launchTool, stationDetail } from "../bridge";
import type { StationDetail } from "../types";

const dialog = () => document.querySelector<HTMLDialogElement>("#station-dialog")!;

export class StationPanel {
  constructor() {
    document.querySelector("#station-close")!.addEventListener("click", () => dialog().close());
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
    if (!node.open) node.showModal();
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
