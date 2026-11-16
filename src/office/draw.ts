import type { BoardMetric, Bot, Todo } from "../types";
import type { Dir } from "./walk";
import { drawText, fitText, textWidth } from "./font";

export const WORLD_W = 480;
export const WORLD_H = 328;

const INK = "#241c17";
const WALL = "#eadcc8";
const WALL_SHADE = "#ddcbb4";
const BASE = "#c4b09a";
const FLOOR = "#d7b48a";
const FLOOR_B = "#cda67c";
const FLOOR_LINE = "#b99262";
const WOOD = "#8d5a3a";
const WOOD_LT = "#c4895a";
const WOOD_DK = "#5c3b28";
const CREAM = "#f7f3ea";
const PAPER = "#fbf7f0";
const SAGE = "#7e9474";
const SAGE_DK = "#4d684f";
const TERRA = "#c45c3e";
const SKIN = "#e6c2a2";

export type Seat = {
  index: number;
  x: number;
  y: number;
  w: number;
  h: number;
};

export const BOARD = { x: 168, y: 12, w: 236, h: 52 };

export type CommRect = { x: number; y: number; w: number; h: number };

const COMM_SPOTS: Record<string, CommRect> = {
  mail: { x: 4, y: 188, w: 42, h: 34 },
  voice: { x: 434, y: 188, w: 42, h: 34 },
  signal: { x: 72, y: 248, w: 52, h: 24 },
  discord: { x: 132, y: 248, w: 56, h: 24 },
  cad: { x: 4, y: 286, w: 74, h: 38 },
  easel: { x: 82, y: 286, w: 68, h: 38 },
  arcade: { x: 154, y: 286, w: 72, h: 38 },
  tv: { x: 230, y: 286, w: 68, h: 38 },
  files: { x: 302, y: 286, w: 80, h: 38 },
  shelf: { x: 386, y: 286, w: 88, h: 38 },
};

export function commRect(id: string): CommRect | null {
  return COMM_SPOTS[id] ?? null;
}

export function layout(count: number): Seat[] {
  const deskW = 72;
  const deskH = 32;
  const gap = 6;
  const rows = [102, 186];
  const counts = [Math.min(6, count), Math.max(0, count - 6)];
  const seats: Seat[] = [];
  let index = 0;
  for (let row = 0; row < rows.length; row += 1) {
    const n = counts[row];
    if (n <= 0) continue;
    const rowW = n * deskW + (n - 1) * gap;
    const start = Math.round((WORLD_W - rowW) / 2);
    for (let col = 0; col < n; col += 1) {
      seats.push({
        index,
        x: start + col * (deskW + gap),
        y: rows[row],
        w: deskW,
        h: deskH,
      });
      index += 1;
    }
  }
  return seats;
}

export function seatHit(seat: Seat): { x: number; y: number; w: number; h: number } {
  return { x: seat.x - 2, y: seat.y - 22, w: seat.w + 4, h: seat.h + 40 };
}

function px(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, color: string) {
  ctx.fillStyle = color;
  ctx.fillRect(x | 0, y | 0, w, h);
}

function disc(ctx: CanvasRenderingContext2D, cx: number, cy: number, r: number, color: string) {
  for (let y = -r; y <= r; y += 1) {
    const span = Math.round(Math.sqrt(Math.max(0, r * r - y * y)));
    px(ctx, cx - span, cy + y, span * 2 + 1, 1, color);
  }
}

export type Actor = {
  bot: Bot;
  x: number;
  y: number;
  dir: Dir;
  step: number;
  line: string;
};

export type CommActor = {
  id: string;
  label: string;
  unread: number | null;
  selected: boolean;
};

export type SceneFrame = {
  bots: Bot[];
  actors: Actor[];
  comms: CommActor[];
  statuses: Record<string, string>;
  todos: Todo[];
  metrics: BoardMetric[];
  updated: string | null;
  focus: string | null;
  homeLine: string;
  selectedId: string | null;
  frame: number;
};

export function paint(ctx: CanvasRenderingContext2D, scene: SceneFrame) {
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, WORLD_W, WORLD_H);
  paintRoom(ctx);
  paintBoard(ctx, scene);
  const seats = layout(scene.bots.length);
  scene.bots.forEach((bot, index) => {
    const seat = seats[index];
    if (!seat) return;
    paintDesk(ctx, seat, bot, scene.statuses[bot.id] ?? "idle", scene.frame, scene.selectedId === bot.id);
  });
  for (const actor of scene.actors) {
    paintPersonAt(ctx, actor.x, actor.y, actor.bot, actor.dir, actor.step);
    if (actor.line && actor.bot.id !== scene.selectedId) paintCloud(ctx, actor.x + 9, actor.y, actor.line);
  }
  paintDecor(ctx);
  for (const comm of scene.comms) paintComm(ctx, comm);
}

function paintRoom(ctx: CanvasRenderingContext2D) {
  px(ctx, 0, 0, WORLD_W, 72, WALL);
  px(ctx, 0, 0, WORLD_W, 8, WALL_SHADE);
  px(ctx, 0, 66, WORLD_W, 6, BASE);
  px(ctx, 0, 72, WORLD_W, WORLD_H - 72, FLOOR);
  for (let y = 86; y < WORLD_H; y += 12) {
    px(ctx, 0, y, WORLD_W, 1, y % 24 === 14 ? FLOOR_LINE : FLOOR_B);
  }
  paintWindow(ctx, 16, 14);
  paintMark(ctx, 112, 36);
  paintShelf(ctx, 424, 18);
}

function paintWindow(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x, y, 44, 46, WOOD_DK);
  px(ctx, x + 3, y + 3, 38, 40, "#f4efe4");
  px(ctx, x + 3, y + 3, 38, 10, "#fbf7f0");
  px(ctx, x + 21, y + 3, 2, 40, WOOD);
  px(ctx, x + 3, y + 22, 38, 2, WOOD);
  px(ctx, x + 16, y + 46, 12, 4, WOOD);
}

function paintMark(ctx: CanvasRenderingContext2D, cx: number, cy: number) {
  disc(ctx, cx, cy, 13, INK);
  disc(ctx, cx, cy, 11, TERRA);
  disc(ctx, cx, cy, 6, CREAM);
  disc(ctx, cx, cy, 3, WALL);
}

function paintShelf(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x, y + 28, 40, 4, WOOD_DK);
  px(ctx, x + 4, y + 6, 6, 22, TERRA);
  px(ctx, x + 12, y + 10, 5, 18, SAGE_DK);
  px(ctx, x + 19, y + 4, 7, 24, WOOD);
  px(ctx, x + 28, y + 12, 5, 16, "#c47b86");
}

function paintDecor(ctx: CanvasRenderingContext2D) {
  px(ctx, 196, 246, 92, 22, TERRA);
  px(ctx, 200, 249, 84, 16, CREAM);
  px(ctx, 204, 252, 76, 10, "#a34a36");
  px(ctx, 22, 236, 16, 10, WOOD);
  px(ctx, 24, 244, 12, 8, WOOD_DK);
  px(ctx, 18, 228, 8, 10, SAGE);
  px(ctx, 26, 222, 10, 12, SAGE_DK);
  px(ctx, 30, 230, 8, 8, SAGE);
}

function paintComm(ctx: CanvasRenderingContext2D, comm: CommActor) {
  const spot = commRect(comm.id);
  if (!spot) return;
  if (comm.selected) {
    px(ctx, spot.x - 2, spot.y - 2, spot.w + 4, spot.h + 4, PAPER);
  }
  if (comm.id === "mail") paintTray(ctx, spot.x + 4, spot.y);
  else if (comm.id === "voice") paintPhone(ctx, spot.x + 6, spot.y);
  else if (comm.id === "signal") paintNote(ctx, spot.x + 2, spot.y);
  else if (comm.id === "discord") paintSpeaker(ctx, spot.x + 2, spot.y);
  else if (comm.id === "cad") paintBench(ctx, spot.x + 1, spot.y);
  else if (comm.id === "easel") paintEasel(ctx, spot.x + 28, spot.y);
  else if (comm.id === "arcade") paintCabinet(ctx, spot.x + 30, spot.y);
  else if (comm.id === "tv") paintTv(ctx, spot.x + 12, spot.y);
  else if (comm.id === "files") paintDrawers(ctx, spot.x + 22, spot.y);
  else if (comm.id === "shelf") paintShelfStack(ctx, spot.x + 24, spot.y);
  const plateY = spot.y + spot.h - 11;
  px(ctx, spot.x, plateY, spot.w, 11, WOOD_DK);
  px(ctx, spot.x + 1, plateY + 1, spot.w - 2, 9, "#f3eadc");
  const label = fitText(comm.label, spot.w - 4);
  drawText(ctx, label, spot.x + Math.round((spot.w - textWidth(label)) / 2), plateY + 3, INK);
  if (comm.unread && comm.unread > 0) {
    const text = comm.unread > 9 ? "9+" : String(comm.unread);
    const width = textWidth(text) + 4;
    const bx = spot.x + spot.w - width;
    const by = spot.y - 2;
    px(ctx, bx, by, width, 7, TERRA);
    drawText(ctx, text, bx + 2, by + 1, PAPER);
  }
}

function paintTray(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x, y + 8, 34, 12, WOOD);
  px(ctx, x, y + 8, 34, 2, WOOD_LT);
  px(ctx, x + 2, y + 10, 30, 8, WOOD_DK);
  px(ctx, x + 4, y + 4, 22, 8, PAPER);
  px(ctx, x + 8, y + 2, 20, 8, CREAM);
  px(ctx, x + 12, y, 16, 8, PAPER);
  px(ctx, x + 14, y + 2, 10, 1, TERRA);
}

function paintPhone(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x + 2, y + 12, 26, 8, WOOD_DK);
  px(ctx, x + 4, y + 8, 22, 6, WOOD);
  px(ctx, x, y + 2, 8, 6, INK);
  px(ctx, x + 22, y + 2, 8, 6, INK);
  px(ctx, x + 2, y, 26, 4, INK);
  px(ctx, x + 12, y + 10, 4, 3, CREAM);
  px(ctx, x + 8, y + 16, 2, 3, WOOD);
}

function paintNote(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x + 4, y, 16, 12, PAPER);
  px(ctx, x + 4, y, 16, 2, CREAM);
  px(ctx, x + 6, y + 4, 10, 1, WOOD);
  px(ctx, x + 6, y + 7, 8, 1, WOOD);
  disc(ctx, x + 16, y + 9, 3, TERRA);
}

function paintBench(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x, y + 14, 72, 8, WOOD);
  px(ctx, x, y + 14, 72, 2, WOOD_LT);
  px(ctx, x + 4, y + 22, 4, 4, WOOD_DK);
  px(ctx, x + 64, y + 22, 4, 4, WOOD_DK);
  px(ctx, x + 6, y + 6, 22, 8, PAPER);
  px(ctx, x + 8, y + 8, 14, 1, INK);
  px(ctx, x + 8, y + 11, 10, 1, WOOD);
  px(ctx, x + 36, y + 2, 16, 12, INK);
  px(ctx, x + 38, y + 4, 12, 8, "#d7e4ef");
  px(ctx, x + 56, y + 8, 10, 6, CREAM);
  px(ctx, x + 58, y + 8, 8, 2, TERRA);
}

function paintEasel(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x + 14, y + 2, 2, 22, WOOD_DK);
  px(ctx, x + 2, y + 8, 26, 2, WOOD);
  px(ctx, x + 4, y, 20, 16, PAPER);
  px(ctx, x + 6, y + 3, 8, 6, SAGE);
  px(ctx, x + 14, y + 6, 6, 6, TERRA);
}

function paintCabinet(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x, y, 28, 24, WOOD_DK);
  px(ctx, x + 3, y + 3, 22, 12, INK);
  px(ctx, x + 5, y + 5, 18, 8, "#3d6b8a");
  px(ctx, x + 6, y + 17, 4, 3, TERRA);
  px(ctx, x + 14, y + 17, 4, 3, SAGE);
  px(ctx, x + 8, y + 22, 12, 2, WOOD);
}

function paintDrawers(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x, y + 2, 30, 20, WOOD);
  px(ctx, x, y + 2, 30, 2, WOOD_LT);
  px(ctx, x, y + 8, 30, 1, WOOD_DK);
  px(ctx, x, y + 14, 30, 1, WOOD_DK);
  px(ctx, x + 12, y + 4, 6, 2, CREAM);
  px(ctx, x + 12, y + 10, 6, 2, CREAM);
  px(ctx, x + 12, y + 16, 6, 2, CREAM);
}

function paintShelfStack(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x, y + 8, 40, 3, WOOD_DK);
  px(ctx, x, y + 18, 40, 3, WOOD);
  px(ctx, x + 4, y + 2, 6, 6, TERRA);
  px(ctx, x + 12, y + 3, 5, 5, SAGE);
  px(ctx, x + 20, y + 1, 7, 7, PAPER);
  px(ctx, x + 30, y + 11, 6, 7, INK);
}

function paintTv(ctx: CanvasRenderingContext2D, x: number, y: number) {
  px(ctx, x, y + 2, 40, 18, INK);
  px(ctx, x + 3, y + 5, 34, 12, "#1e3344");
  px(ctx, x + 6, y + 8, 10, 6, TERRA);
  px(ctx, x + 18, y + 8, 8, 4, CREAM);
  px(ctx, x + 16, y + 20, 8, 3, WOOD_DK);
  px(ctx, x + 8, y + 23, 24, 2, WOOD);
}

function paintSpeaker(ctx: CanvasRenderingContext2D, x: number, y: number) {
  disc(ctx, x + 10, y + 6, 6, WOOD_DK);
  disc(ctx, x + 10, y + 6, 4, INK);
  disc(ctx, x + 10, y + 6, 2, CREAM);
  px(ctx, x + 8, y + 12, 4, 3, WOOD);
}

function paintBoard(ctx: CanvasRenderingContext2D, scene: SceneFrame) {
  const { x, y, w, h } = BOARD;
  px(ctx, x - 3, y - 3, w + 6, h + 6, WOOD_DK);
  px(ctx, x, y, w, h, PAPER);
  px(ctx, x, y + h - 2, w, 2, "#e7dccb");
  drawText(ctx, "BOARD", x + 4, y + 3, WOOD);
  const bottom = y + h - (scene.updated ? 8 : 3);
  let cursor = y + 11;
  if (scene.metrics.length) {
    cursor = paintMetrics(ctx, scene.metrics, x + 4, cursor, w - 8, bottom);
  } else if (scene.focus) {
    drawText(ctx, fitText(scene.focus, w - 10), x + 4, cursor, WOOD);
    cursor += 8;
  }
  if (scene.homeLine) {
    drawText(ctx, fitText(scene.homeLine, w - 10), x + 4, cursor, TERRA);
    cursor += 8;
  }
  const open = scene.todos.filter((item) => !item.done);
  const room = Math.max(0, Math.floor((bottom - cursor) / 8));
  const shown = (open.length ? open : scene.todos).slice(0, room);
  if (shown.length === 0 && !scene.metrics.length && !scene.focus) {
    drawText(ctx, "THE BOARD IS CLEAR", x + 4, y + 18, SAGE_DK);
  }
  shown.forEach((item, index) => {
    const ly = cursor + index * 8;
    paintTaskDot(ctx, x + 4, ly + 1, item);
    drawText(ctx, fitText(item.text, w - 16), x + 12, ly, item.done ? "#8a7768" : INK);
  });
  if (scene.updated) {
    drawText(ctx, fitText(scene.updated, w - 8), x + 4, y + h - 7, "#8a7768");
  }
}

function paintMetrics(
  ctx: CanvasRenderingContext2D,
  metrics: BoardMetric[],
  x: number,
  y: number,
  width: number,
  bottom: number,
): number {
  let cx = x;
  let cy = y;
  const rowH = 13;
  for (const metric of metrics) {
    const value = fitText(metric.value, width);
    const label = fitText(metric.label, width);
    const tile = Math.min(width, Math.max(textWidth(value), textWidth(label)) + 2);
    if (cx > x && cx + tile > x + width) {
      cx = x;
      cy += rowH;
    }
    if (cy + 11 > bottom) break;
    drawText(ctx, value, cx, cy, metric.bad ? TERRA : INK);
    drawText(ctx, label, cx, cy + 6, WOOD);
    cx += tile + 4;
  }
  return cy + rowH;
}

function paintTaskDot(ctx: CanvasRenderingContext2D, x: number, y: number, item: Todo) {
  if (item.done) {
    px(ctx, x, y, 5, 5, SAGE);
    px(ctx, x + 1, y + 2, 3, 1, PAPER);
    return;
  }
  if (item.today) {
    disc(ctx, x + 2, y + 2, 2, TERRA);
    return;
  }
  disc(ctx, x + 2, y + 2, 2, TERRA);
  disc(ctx, x + 2, y + 2, 1, PAPER);
}

function paintCloud(ctx: CanvasRenderingContext2D, cx: number, top: number, text: string) {
  const label = fitText(text, 78);
  const width = textWidth(label) + 8;
  const left = Math.round(cx - width / 2);
  const y = top - 14;
  px(ctx, left, y, width, 9, PAPER);
  px(ctx, left, y + 9, width, 1, WOOD);
  px(ctx, cx - 1, y + 10, 2, 2, PAPER);
  drawText(ctx, label, left + 3, y + 2, INK);
}

function paintPersonAt(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  bot: Bot,
  dir: Dir,
  step: number,
) {
  const frame = step === 0 ? 0 : 1;
  const shift = step === 1 ? -1 : step === 2 ? 1 : 0;
  const face = dir === "e" ? 1 : dir === "w" ? -1 : 0;
  const shirt = bot.accent || TERRA;
  const hair = bot.hair || INK;
  px(ctx, x + 3, y + 20, 12, 2, "rgba(36,28,23,0.18)");
  px(ctx, x + 3, y, 12, 5, hair);
  px(ctx, x + 2, y + 1, 14, 4, hair);
  px(ctx, x + 4, y + 4, 10, 8, SKIN);
  if (frame === 0) {
    px(ctx, x + 6 + face, y + 7, 2, 2, INK);
    px(ctx, x + 10 + face, y + 7, 2, 2, INK);
  } else {
    px(ctx, x + 6 + face, y + 8, 2, 1, INK);
    px(ctx, x + 10 + face, y + 8, 2, 1, INK);
  }
  px(ctx, x + 7, y + 10, 4, 1, "#c48b74");
  px(ctx, x + 4, y + 12, 10, 8, shirt);
  px(ctx, x + 2, y + 13, 3, 6, shirt);
  px(ctx, x + 13, y + 13, 3, 6, shirt);
  const handY = 18 + (dir === "n" || dir === "s" ? shift : 0);
  const handX = dir === "e" || dir === "w" ? shift : 0;
  px(ctx, x + 2 + handX, handY, 2, 2, SKIN);
  px(ctx, x + 14 + handX, handY, 2, 2, SKIN);
  paintAccessory(ctx, x, y, bot.accessory, shirt);
}

function paintAccessory(ctx: CanvasRenderingContext2D, x: number, y: number, accessory: string, shirt: string) {
  if (accessory === "star") {
    px(ctx, x + 8, y + 14, 2, 2, PAPER);
    px(ctx, x + 7, y + 15, 4, 1, PAPER);
  } else if (accessory === "leaf") {
    px(ctx, x + 12, y, 4, 3, SAGE);
    px(ctx, x + 13, y + 1, 1, 2, SAGE_DK);
  } else if (accessory === "cup") {
    px(ctx, x + 15, y + 16, 4, 3, PAPER);
    px(ctx, x + 18, y + 17, 2, 1, WOOD);
  } else if (accessory === "brush") {
    px(ctx, x + 15, y + 8, 1, 8, WOOD_DK);
    px(ctx, x + 14, y + 7, 3, 2, TERRA);
  } else if (accessory === "book") {
    px(ctx, x + 5, y + 16, 8, 3, CREAM);
    px(ctx, x + 8, y + 16, 1, 3, TERRA);
  } else if (accessory === "gear") {
    px(ctx, x + 7, y + 15, 4, 4, "#e0a15a");
    px(ctx, x + 8, y + 16, 2, 2, shirt);
  } else if (accessory === "chip") {
    px(ctx, x + 6, y + 14, 6, 4, PAPER);
    px(ctx, x + 7, y + 15, 4, 2, SAGE_DK);
  } else if (accessory === "mask") {
    px(ctx, x + 5, y + 6, 8, 4, "#a35a62");
    px(ctx, x + 6, y + 7, 2, 2, PAPER);
    px(ctx, x + 10, y + 7, 2, 2, PAPER);
  }
}

function paintDesk(
  ctx: CanvasRenderingContext2D,
  seat: Seat,
  bot: Bot,
  status: string,
  frame: number,
  selected: boolean,
) {
  if (selected) {
    px(ctx, seat.x - 2, seat.y - 22, seat.w + 4, 1, PAPER);
    px(ctx, seat.x - 2, seat.y + seat.h + 16, seat.w + 4, 1, PAPER);
    px(ctx, seat.x - 2, seat.y - 22, 1, seat.h + 39, PAPER);
    px(ctx, seat.x + seat.w + 1, seat.y - 22, 1, seat.h + 39, PAPER);
  }
  px(ctx, seat.x + 4, seat.y + seat.h, 3, 5, WOOD_DK);
  px(ctx, seat.x + seat.w - 7, seat.y + seat.h, 3, 5, WOOD_DK);
  px(ctx, seat.x, seat.y, seat.w, seat.h, WOOD);
  px(ctx, seat.x, seat.y, seat.w, 3, WOOD_LT);
  px(ctx, seat.x, seat.y + seat.h - 3, seat.w, 3, WOOD_DK);
  const mx = seat.x + 8;
  const my = seat.y + 5;
  px(ctx, mx, my, 16, 12, INK);
  px(ctx, mx + 1, my + 1, 14, 9, PAPER);
  disc(ctx, mx + 8, my + 5, 2, TERRA);
  px(ctx, mx + 7, my + 12, 3, 2, INK);
  px(ctx, seat.x + 28, seat.y + 16, 18, 5, "#4a3b32");
  px(ctx, seat.x + 30, seat.y + 18, 14, 1, "#6b5344");
  const lx = seat.x + seat.w - 12;
  const ly = seat.y + 5;
  px(ctx, lx + 2, ly + 5, 2, 6, WOOD_DK);
  const lamp = lampColor(status, frame);
  px(ctx, lx, ly, 6, 5, lamp);
  px(ctx, lx + 1, ly + 1, 4, 2, status === "idle" ? "#d5e0cf" : PAPER);
  const plateY = seat.y + seat.h + 6;
  px(ctx, seat.x + 2, plateY, seat.w - 4, 11, WOOD_DK);
  px(ctx, seat.x + 3, plateY + 1, seat.w - 6, 9, "#f3eadc");
  const label = fitText(bot.name, seat.w - 10);
  const tx = seat.x + Math.round((seat.w - textWidth(label)) / 2);
  drawText(ctx, label, tx, plateY + 3, INK);
}

function lampColor(status: string, frame: number): string {
  if (status === "working") return frame === 0 ? "#e0a15a" : "#f6e0b5";
  if (status === "message") return frame === 0 ? "#fffaf3" : "#f3d7a4";
  return SAGE;
}
