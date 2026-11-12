import { setFeature } from "../bridge";
import type { FeatureFlags, OfficeSnapshot, ServiceReport } from "../types";

export class SettingsPanel {
  constructor(private readonly onChange: (snapshot: OfficeSnapshot) => void) {
    document.querySelector("#open-settings")!.addEventListener("click", () => this.open());
    document.querySelector("#quiet-settings")!.addEventListener("click", () => this.open());
    document.querySelector("#settings-close")!.addEventListener("click", () => this.close());
    document.querySelector("#toggle-office")!.addEventListener("click", () => {
      void this.flip("office");
    });
    document.querySelector("#toggle-filter")!.addEventListener("click", () => {
      void this.flip("filter");
    });
    document.querySelector("#toggle-compute")!.addEventListener("click", () => {
      void this.flip("sharedCompute");
    });
    document.querySelector("#compute-off")!.addEventListener("click", () => {
      void this.set("sharedCompute", false);
    });
    document.querySelector("#quiet-office-on")!.addEventListener("click", () => {
      void this.set("office", true);
    });
    document.querySelector("#quiet-compute-off")!.addEventListener("click", () => {
      void this.set("sharedCompute", false);
    });
  }

  open() {
    const dialog = document.querySelector<HTMLDialogElement>("#settings-dialog")!;
    if (!dialog.open) dialog.showModal();
  }

  close() {
    document.querySelector<HTMLDialogElement>("#settings-dialog")!.close();
  }

  render(snapshot: OfficeSnapshot) {
    const flags = snapshot.features;
    this.switchButton("#toggle-office", flags.office);
    this.switchButton("#toggle-filter", flags.filter);
    this.switchButton("#toggle-compute", flags.sharedCompute);
    this.line("#filter-status", snapshot.services.filter);
    this.line("#compute-status", snapshot.services.sharedCompute);
    document.querySelector("#compute-running")!.textContent = snapshot.services.sharedCompute.running;
    document.querySelector("#compute-contributed")!.textContent = snapshot.services.sharedCompute.contributed;
    document.querySelector("#compute-earned")!.textContent = snapshot.services.sharedCompute.earned;
    const off = document.querySelector<HTMLButtonElement>("#compute-off")!;
    off.disabled = !flags.sharedCompute;
    off.textContent = flags.sharedCompute ? "Turn off" : "Off";
    this.renderQuiet(snapshot);
  }

  private renderQuiet(snapshot: OfficeSnapshot) {
    const quiet = document.querySelector<HTMLElement>("#quiet")!;
    const floor = document.querySelector<HTMLElement>("#floor")!;
    const officeOn = snapshot.features.office;
    quiet.hidden = officeOn;
    floor.hidden = !officeOn;
    const list = document.querySelector<HTMLDListElement>("#quiet-readout")!;
    list.replaceChildren();
    list.append(
      cell("Office", officeOn ? "On" : "Off"),
      cell("Filter", snapshot.services.filter.phase === "off" ? "Off" : snapshot.services.filter.phase),
      cell("Shared Compute", snapshot.services.sharedCompute.running === "Nothing" && !snapshot.features.sharedCompute
        ? "Off"
        : snapshot.services.sharedCompute.phase),
    );
    const computeOff = document.querySelector<HTMLButtonElement>("#quiet-compute-off")!;
    computeOff.hidden = !snapshot.features.sharedCompute;
    document.querySelector(".sub")!.textContent = officeOn
      ? "Productive, clean, and earning."
      : "Office is off. Filter and compute stay on their own switches.";
  }

  private switchButton(selector: string, on: boolean) {
    const button = document.querySelector<HTMLButtonElement>(selector)!;
    button.setAttribute("aria-checked", on ? "true" : "false");
    button.textContent = on ? "On" : "Off";
  }

  private line(selector: string, report: ServiceReport) {
    document.querySelector(selector)!.textContent = report.detail;
  }

  private async flip(feature: keyof FeatureFlags) {
    const button = document.querySelector<HTMLButtonElement>(
      feature === "office" ? "#toggle-office" : feature === "filter" ? "#toggle-filter" : "#toggle-compute",
    )!;
    const enabled = button.getAttribute("aria-checked") !== "true";
    await this.set(feature, enabled);
  }

  private async set(feature: keyof FeatureFlags, enabled: boolean) {
    try {
      const snapshot = await setFeature(feature, enabled);
      this.onChange(snapshot);
    } catch (error) {
      const lede = document.querySelector("#settings-lede")!;
      lede.textContent = error instanceof Error ? error.message : "That switch did not save.";
    }
  }
}

function cell(label: string, value: string): HTMLDivElement {
  const wrap = document.createElement("div");
  const dt = document.createElement("dt");
  dt.textContent = label;
  const dd = document.createElement("dd");
  dd.textContent = value;
  wrap.append(dt, dd);
  return wrap;
}
