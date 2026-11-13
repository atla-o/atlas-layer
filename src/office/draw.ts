import type { BoardMetric, Bot, Todo } from "../types";
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

export type HotKind = "computer" | "papers" | "drive" | "mug" | "head";

export type Hotspot = {
  kind: HotKind;
  id: string;
  x: number;
  y: number;
  w: number;
  h: number;
  href?: string;
};

function linkOf(bot: Bot, key: keyof NonNullable<Bot["links"]>): string | undefined {
  const value = bot.links?.[key];
  const text = typeof value === "string" ? value.trim() : "";
  return text || undefined;
}

/** Clickable desk objects. Items with no action are omitted. */
export function hotspots(bots: Bot[]): Hotspot[] {
  const seats = layout(bots.length);
  const spots: Hotspot[] = [];
  bots.forEach((bot, index) => {
    const seat = seats[index];
    if (!seat) return;
    const computer =
      linkOf(bot, "computer") ||
      (bot.connector === "cursor-agents" && bot.agentId
        ? `https://cursor.com/agents/${bot.agentId}`
        : undefined);
    spots.push({
      kind: "computer",
      id: bot.id,
      x: seat.x + 8,
      y: seat.y + 4,
      w: 16,
      h: 16,
      href: computer,
    });
    const papers = linkOf(bot, "papers");
    if (papers) {
      spots.push({ kind: "papers", id: bot.id, x: seat.x + 26, y: seat.y + 6, w: 14, h: 10, href: papers });
    }
    const drive = linkOf(bot, "drive");
    if (drive) {
      spots.push({ kind: "drive", id: bot.id, x: seat.x + 42, y: seat.y + 6, w: 12, h: 10, href: drive });
    }
    const mug = linkOf(bot, "mug");
    if (mug) {
      spots.push({ kind: "mug", id: bot.id, x: seat.x + seat.w - 20, y: seat.y + 16, w: 10, h: 8, href: mug });
    }
    const headX = seat.x + Math.round((seat.w - 18) / 2);
    spots.push({ kind: "head", id: bot.id, x: headX, y: seat.y - 18, w: 18, h: 22 });
  });
  return spots;
}

export function headPoint(bots: Bot[], id: string): { x: number; y: number } | null {
  const index = bots.findIndex((bot) => bot.id === id);
  if (index < 0) return null;
  const seat = layout(bots.length)[index];
  if (!seat) return null;
  return {
    x: seat.x + Math.round(seat.w / 2),
    y: seat.y - 18,
  };
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
  metrics: BoardMetric[];
  updated: string | null;
  focus: string | null;
  selectedId: string | null;
  hover: Hotspot | null;
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
    paintPerson(ctx, seat, bot, scene.frame);
    paintDesk(
      ctx,
      seat,
      bot,
      scene.statuses[bot.id] ?? "idle",
      scene.frame,
      scene.selectedId === bot.id,
      scene.hover?.id === bot.id ? scene.hover : null,
    );
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
  hover: Hotspot | null,
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
  if (hover?.kind === "computer") px(ctx, mx - 1, my - 1, 18, 16, PAPER);
  px(ctx, mx, my, 16, 12, INK);
  px(ctx, mx + 1, my + 1, 14, 9, PAPER);
  disc(ctx, mx + 8, my + 5, 2, TERRA);
  px(ctx, mx + 7, my + 12, 3, 2, INK);
  if (bot.links?.papers) {
    const px0 = seat.x + 26;
    const py0 = seat.y + 6;
    if (hover?.kind === "papers") px(ctx, px0 - 1, py0 - 1, 16, 12, PAPER);
    px(ctx, px0, py0, 14, 10, CREAM);
    px(ctx, px0 + 2, py0 + 2, 10, 1, WOOD);
    px(ctx, px0 + 2, py0 + 5, 8, 1, WOOD);
  }
  if (bot.links?.drive) {
    const dx = seat.x + 42;
    const dy = seat.y + 6;
    if (hover?.kind === "drive") px(ctx, dx - 1, dy - 1, 14, 12, PAPER);
    px(ctx, dx, dy + 2, 12, 8, "#6e8fbe");
    px(ctx, dx + 1, dy, 6, 3, "#8eadd4");
  }
  if (bot.links?.mug) {
    const ux = seat.x + seat.w - 20;
    const uy = seat.y + 16;
    if (hover?.kind === "mug") px(ctx, ux - 1, uy - 1, 12, 10, PAPER);
    px(ctx, ux, uy + 2, 8, 6, PAPER);
    px(ctx, ux + 7, uy + 3, 2, 3, WOOD);
  }
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
