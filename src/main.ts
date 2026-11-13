import { loadOffice, onOfficeEvent, openExternal, watchFocus, watchOffice } from "./bridge";
import { OfficeScene } from "./office/scene";
import type { OfficeEvent, OfficeSnapshot } from "./types";
import { Board } from "./ui/board";
import { ChatPanel } from "./ui/chat";
import { SettingsPanel } from "./ui/settings";

const banner = document.querySelector<HTMLElement>("#banner")!;
const note = document.querySelector<HTMLElement>("#floor-note")!;
const canvas = document.querySelector<HTMLCanvasElement>("#office")!;
const floor = document.querySelector<HTMLElement>("#floor")!;

let snapshot: OfficeSnapshot | null = null;

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
  (href) => {
    void openExternal(href);
  },
  (point) => chat.place(point),
);

const board = new Board((next) => applySnapshot(next));
const chat = new ChatPanel((id, status) => {
  snapshot!.statuses[id] = status;
  scene.setStatus(id, status);
});
const settings = new SettingsPanel((next) => applySnapshot(next));

let officeOn = true;

function applySnapshot(next: OfficeSnapshot) {
  const wasOn = officeOn;
  snapshot = next;
  officeOn = next.features.office;
  document.querySelector("#title")!.textContent = "Atlas Layer";
  document.title = "Atlas Layer";
  scene.setBots(next.bots, next.statuses);
  scene.setBoard(next.todos, next.metrics, next.updated, next.todayFocus);
  board.set(next);
  settings.render(next);
  note.hidden = true;
  if (!officeOn) {
    scene.setRunning(false);
    scene.select(null);
    if (wasOn && chat.openId) void chat.close();
    document.querySelector<HTMLDialogElement>("#board-dialog")!.close();
  } else if (!document.hidden) {
    scene.setRunning(true);
  }
  if (next.preview) showBanner("Browser preview. The installed app is the one that reaches Ollama, the relay, and Cursor.", "preview");
  else if (next.warnings.length) showBanner(next.warnings[0], "error");
  else hideBanner();
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
    scene.setBoard(event.todos, event.metrics, event.updated, event.todayFocus);
    board.set(snapshot);
  } else if (event.type === "warning") {
    showBanner(event.message, "error");
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
  if (event.key !== "Escape") return;
  const dialog = document.querySelector<HTMLDialogElement>("#board-dialog")!;
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
