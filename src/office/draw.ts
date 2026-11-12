import type { Bot, Todo } from "../types";
import { drawText, fitText, textWidth } from "./font";

export const WORLD_W = 480;
export const WORLD_H = 276;

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

export type SceneFrame = {
  bots: Bot[];
  statuses: Record<string, string>;
  todos: Todo[];
  selectedId: string | null;
  frame: number;
};

export function paint(ctx: CanvasRenderingContext2D, scene: SceneFrame) {
  ctx.imageSmoothingEnabled = false;
  ctx.clearRect(0, 0, WORLD_W, WORLD_H);
  paintRoom(ctx);
  paintBoard(ctx, scene.todos);
  const seats = layout(scene.bots.length);
  scene.bots.forEach((bot, index) => {
    const seat = seats[index];
    if (!seat) return;
    paintPerson(ctx, seat, bot, scene.frame);
    paintDesk(ctx, seat, bot, scene.statuses[bot.id] ?? "idle", scene.frame, scene.selectedId === bot.id);
  });
  paintDecor(ctx);
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

function paintBoard(ctx: CanvasRenderingContext2D, todos: Todo[]) {
  const { x, y, w, h } = BOARD;
  px(ctx, x - 3, y - 3, w + 6, h + 6, WOOD_DK);
  px(ctx, x, y, w, h, PAPER);
  px(ctx, x, y + h - 2, w, 2, "#e7dccb");
  drawText(ctx, "BOARD", x + 6, y + 4, WOOD);
  const open = todos.filter((item) => !item.done);
  const lines = (open.length ? open : todos).slice(0, 4);
  if (lines.length === 0) {
    drawText(ctx, "THE BOARD IS CLEAR", x + 6, y + 18, SAGE_DK);
    return;
  }
  lines.forEach((item, index) => {
    const ly = y + 16 + index * 8;
    px(ctx, x + 6, ly + 1, 5, 5, item.done ? SAGE : WOOD_LT);
    if (item.done) {
      px(ctx, x + 7, ly + 3, 3, 1, PAPER);
    }
    const label = fitText(item.text, w - 22);
    drawText(ctx, label, x + 14, ly, item.done ? "#8a7768" : INK);
  });
}

function paintPerson(ctx: CanvasRenderingContext2D, seat: Seat, bot: Bot, frame: number) {
  const x = seat.x + Math.round((seat.w - 18) / 2);
  const y = seat.y - 18;
  const shirt = bot.accent || TERRA;
  const hair = bot.hair || INK;
  px(ctx, x + 3, y + 20, 12, 2, "rgba(36,28,23,0.18)");
  px(ctx, x + 3, y, 12, 5, hair);
  px(ctx, x + 2, y + 1, 14, 4, hair);
  px(ctx, x + 4, y + 4, 10, 8, SKIN);
  if (frame === 0) {
    px(ctx, x + 6, y + 7, 2, 2, INK);
    px(ctx, x + 10, y + 7, 2, 2, INK);
  } else {
    px(ctx, x + 6, y + 8, 2, 1, INK);
    px(ctx, x + 10, y + 8, 2, 1, INK);
  }
  px(ctx, x + 7, y + 10, 4, 1, "#c48b74");
  px(ctx, x + 4, y + 12, 10, 8, shirt);
  px(ctx, x + 2, y + 13, 3, 6, shirt);
  px(ctx, x + 13, y + 13, 3, 6, shirt);
  const handY = frame === 0 ? 18 : 17;
  px(ctx, x + 2, handY, 2, 2, SKIN);
  px(ctx, x + 14, handY, 2, 2, SKIN);
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
