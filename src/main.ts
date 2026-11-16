import { loadOffice, onOfficeEvent, openComms, openThing, watchFocus, watchOffice } from "./bridge";
import { OfficeScene } from "./office/scene";
import type { CommObject, OfficeEvent, OfficeSnapshot } from "./types";
import { Board } from "./ui/board";
import { ChatPanel } from "./ui/chat";
import { SettingsPanel } from "./ui/settings";
import { StationPanel } from "./ui/station";
import { TvFinder } from "./ui/tv";

const banner = document.querySelector<HTMLElement>("#banner")!;
const note = document.querySelector<HTMLElement>("#floor-note")!;
const canvas = document.querySelector<HTMLCanvasElement>("#office")!;
const floor = document.querySelector<HTMLElement>("#floor")!;

let snapshot: OfficeSnapshot | null = null;

const board = new Board((next) => applySnapshot(next));
const chat = new ChatPanel(
  (id, status) => {
    snapshot!.statuses[id] = status;
    scene.setStatus(id, status);
  },
  (id, text) => scene.setLine(id, text),
);
const scene = new OfficeScene(
  canvas,
  floor,
  (id) => {
    const bot = snapshot?.bots.find((item) => item.id === id);
    if (!bot) return;
    scene.select(id);
    void chat.open(bot);
  },
  () => board.open(),
  (point) => chat.place(point),
  (id) => {
    if (id === "mail" || id === "voice" || id === "signal" || id === "discord") {
      void openComms(id).catch((error: unknown) => {
        showBanner(error instanceof Error ? error.message : "That did not open.", "error");
      });
    }
  },
  (thing) => {
    void openThing(thing.app, thing.target).catch((error: unknown) => {
      showBanner(error instanceof Error ? error.message : "That did not open.", "error");
    });
  },
);
const stations = new StationPanel();
const tv = new TvFinder(() => scene.screenBox("tv"), (message) => showBanner(message, "error"));
const settings = new SettingsPanel((next) => applySnapshot(next));

let officeOn = true;

function applySnapshot(next: OfficeSnapshot) {
  const wasOn = officeOn;
  snapshot = next;
  officeOn = next.features.office;
  document.querySelector("#title")!.textContent = "Atlas Layer";
  document.title = "Atlas Layer";
  scene.setBots(next.bots, next.statuses);
  scene.setComms(objectsOnTheFloor(next));
  scene.setThings(next.things ?? []);
  tv.sync(Boolean(next.stremio) && (next.stations ?? []).some((spot) => spot.id === "tv"));
  if (next.preview) {
    scene.setLine("atlas", "Three bills, two replies waiting, and one decision about the shelf.");
    scene.setLine("products", "Shelf sketch is on the desk.");
    scene.setLine("consumed", "Finished the chapter.");
  }
  scene.setBoard(next.todos, next.metrics, next.updated, next.todayFocus, homeLine(next));
  board.set(next);
  settings.render(next);
  note.hidden = true;
  if (!officeOn) {
    scene.setRunning(false);
    scene.select(null);
    if (wasOn && chat.openId) void chat.close();
    document.querySelector<HTMLDialogElement>("#board-dialog")!.close();
    document.querySelector<HTMLDialogElement>("#station-dialog")!.close();
  } else if (!document.hidden) {
    scene.setRunning(true);
  }
  if (next.preview) showBanner("Browser preview. The installed app is the one that reaches Ollama, the relay, and Cursor.", "preview");
  else if (next.warnings.length) showBanner(next.warnings[0], "error");
  else hideBanner();
}

const PLATE: Record<string, string> = {
  cad: "CAD",
  easel: "Easel",
  arcade: "Arcade",
  tv: "TV",
  files: "Files",
};

function objectsOnTheFloor(next: OfficeSnapshot): CommObject[] {
  const spots = (next.stations ?? []).map((station) => ({
    id: station.id,
    label: PLATE[station.id] ?? station.label,
    mode: "local",
  }));
  return [...(next.comms ?? []), ...spots];
}

function homeLine(snapshot: OfficeSnapshot): string {
  const bills = snapshot.bills.filter((bill) => !bill.paid).length;
  const parts = [
    bills ? `${bills} BILL${bills === 1 ? "" : "S"}` : "",
    snapshot.replies.length ? `${snapshot.replies.length} REPL${snapshot.replies.length === 1 ? "Y" : "IES"}` : "",
    snapshot.decisions.length ? `${snapshot.decisions.length} DECISION${snapshot.decisions.length === 1 ? "" : "S"}` : "",
  ].filter(Boolean);
  return parts.join("  ");
}

function showBanner(text: string, kind: "error" | "preview") {
  banner.hidden = false;
  banner.dataset.kind = kind;
  banner.textContent = text;
}

function hideBanner() {
  banner.hidden = true;
  banner.textContent = "";
}

function onEvent(event: OfficeEvent) {
  if (!snapshot) return;
  if (event.type === "status") {
    snapshot.statuses[event.botId] = event.status;
    scene.setStatus(event.botId, event.status);
  } else if (event.type === "todos") {
    snapshot.todos = event.todos;
    snapshot.todayFocus = event.todayFocus;
    snapshot.metrics = event.metrics;
    snapshot.updated = event.updated;
    scene.setBoard(event.todos, event.metrics, event.updated, event.todayFocus, homeLine(snapshot));
    board.set(snapshot);
  } else if (event.type === "home") {
    snapshot.bills = event.bills;
    snapshot.replies = event.replies;
    snapshot.decisions = event.decisions;
    snapshot.dashboardUpdated = event.updated;
    scene.setBoard(snapshot.todos, snapshot.metrics, snapshot.updated, snapshot.todayFocus, homeLine(snapshot));
    board.set(snapshot);
  } else if (event.type === "warning") {
    showBanner(event.message, "error");
  } else if (event.type === "badge") {
    scene.setBadge(event.id, event.unread);
  } else if (event.type === "message") {
    scene.setLine(event.botId, event.message.text);
    chat.apply(event);
  } else if (event.type === "done") {
    scene.setLine(event.botId, event.text);
    chat.apply(event);
  } else {
    chat.apply(event);
  }
}

function tickClock() {
  const now = new Date();
  const clock = document.querySelector<HTMLTimeElement>("#clock")!;
  clock.dateTime = now.toISOString();
  clock.textContent = now.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

tickClock();
window.setInterval(tickClock, 30_000);

document.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
    const typing = document.activeElement instanceof HTMLInputElement
      || document.activeElement instanceof HTMLTextAreaElement;
    if (!typing) {
      event.preventDefault();
      void stations.open("shelf");
    }
    return;
  }
  if (event.key !== "Escape") return;
  const dialog = document.querySelector<HTMLDialogElement>("#board-dialog")!;
  const stationDialog = document.querySelector<HTMLDialogElement>("#station-dialog")!;
  if (stationDialog.open) {
    stationDialog.close();
    return;
  }
  if (dialog.open) {
    dialog.close();
    return;
  }
  if (chat.openId) {
    scene.select(null);
    void chat.close();
  }
});

void (async () => {
  try {
    await watchOffice();
    onOfficeEvent(onEvent);
    await watchFocus((focused) => scene.setRunning(officeOn && focused));
    applySnapshot(await loadOffice());
  } catch (error) {
    note.textContent = error instanceof Error ? error.message : "The office did not open.";
  }
})();
