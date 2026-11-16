import type { BoardMetric, Bot, Todo } from "../types";
import type { CommObject } from "../types";
import { BOARD, commRect, layout, paint, seatHit, WORLD_H, WORLD_W, type Actor, type SceneFrame } from "./draw";
import { blockedTiles, createWalker, homeTile, snap, stepWalker, tileKey, wanderSpots, TILE, type Point, type Walker } from "./walk";

export class OfficeScene {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private frame = 0;
  private timer = 0;
  private running = true;
  private bots: Bot[] = [];
  private statuses: Record<string, string> = {};
  private todos: Todo[] = [];
  private metrics: BoardMetric[] = [];
  private updated: string | null = null;
  private focus: string | null = null;
  private homeLine = "";
  private selectedId: string | null = null;
  private walkers = new Map<string, Walker>();
  private lines = new Map<string, string>();
  private comms: CommObject[] = [];
  private badges = new Map<string, number>();
  private selectedComm: string | null = null;
  private reduceMotion: boolean;

  constructor(
    canvas: HTMLCanvasElement,
    private readonly host: HTMLElement,
    private readonly onBot: (id: string) => void,
    private readonly onBoard: () => void,
    private readonly onAnchor: (point: { x: number; y: number } | null) => void,
    private readonly onComm: (id: string) => void,
  ) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("The office canvas is unavailable.");
    this.ctx = ctx;
    this.reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    canvas.addEventListener("click", (event) => this.click(event));
    canvas.addEventListener("mousemove", (event) => this.hover(event));
    new ResizeObserver(() => this.fit()).observe(host);
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) this.pause();
      else this.resume();
    });
    this.fit();
    this.draw();
    this.resume();
  }

  setBots(bots: Bot[], statuses: Record<string, string>) {
    this.bots = bots;
    this.statuses = { ...statuses };
    this.syncWalkers();
    this.draw();
  }

  setLine(id: string, text: string) {
    this.lines.set(id, text);
    this.draw();
  }

  setComms(comms: CommObject[]) {
    this.comms = comms.filter((item) => commRect(item.id));
    this.syncWalkers();
    this.draw();
  }

  setBadge(id: string, unread: number) {
    if (unread > 0) this.badges.set(id, unread);
    else this.badges.delete(id);
    this.draw();
  }

  setBoard(
    todos: Todo[],
    metrics: BoardMetric[],
    updated: string | null,
    focus: string | null,
    homeLine = "",
  ) {
    this.todos = todos;
    this.metrics = metrics;
    this.updated = updated;
    this.focus = focus;
    this.homeLine = homeLine;
    this.draw();
  }

  setStatus(id: string, status: string) {
    this.statuses[id] = status;
    this.draw();
  }

  select(id: string | null) {
    this.selectedId = id;
    this.draw();
  }

  setRunning(running: boolean) {
    if (running) this.resume();
    else this.pause();
  }

  private scene(): SceneFrame {
    return {
      bots: this.bots,
      statuses: this.statuses,
      todos: this.todos,
      metrics: this.metrics,
      updated: this.updated,
      focus: this.focus,
      homeLine: this.homeLine,
      selectedId: this.selectedId,
      actors: this.actors(),
      comms: this.comms.map((item) => ({
        id: item.id,
        label: item.label,
        unread: this.badges.get(item.id) ?? null,
        selected: this.selectedComm === item.id,
      })),
      frame: this.frame,
    };
  }

  private actors(): Actor[] {
    return this.bots.flatMap((bot) => {
      const walker = this.walkers.get(bot.id);
      if (!walker) return [];
      return [{
        bot,
        x: walker.x,
        y: walker.y,
        dir: walker.dir,
        step: walker.step,
        line: this.lines.get(bot.id) ?? "",
      }];
    });
  }

  anchor(id: string | null): { x: number; y: number } | null {
    if (!id) return null;
    const walker = this.walkers.get(id);
    if (!walker) return null;
    return { x: walker.x + 9, y: walker.y };
  }

  private syncWalkers() {
    const seats = layout(this.bots.length);
    const blocked = blockedTiles(seats);
    for (const item of this.comms) {
      const spot = commRect(item.id);
      if (!spot) continue;
      for (let y = spot.y; y < spot.y + spot.h; y += TILE) {
        for (let x = spot.x; x < spot.x + spot.w; x += TILE) {
          blocked.add(tileKey(snap(x, y)));
        }
      }
    }
    const spots = wanderSpots(blocked);
    this.bots.forEach((bot, index) => {
      const seat = seats[index];
      if (!seat) return;
      const home = homeTile(seat, blocked);
      const existing = this.walkers.get(bot.id);
      if (!existing) this.walkers.set(bot.id, createWalker(bot.id, home));
      else existing.home = home;
    });
    for (const id of [...this.walkers.keys()]) {
      if (!this.bots.some((bot) => bot.id === id)) this.walkers.delete(id);
    }
    this.spots = spots;
    this.blocked = blocked;
  }

  private spots: Point[] = [];
  private blocked = new Set<string>();

  private stepWalkers() {
    if (this.reduceMotion) return;
    const seats = layout(this.bots.length);
    this.bots.forEach((bot, index) => {
      const walker = this.walkers.get(bot.id);
      const seat = seats[index];
      if (!walker || !seat) return;
      const goHome = this.selectedId === bot.id || this.statuses[bot.id] === "working";
      const roll = (this.frame * 5 + index * 3) % 19;
      this.walkers.set(bot.id, stepWalker(walker, goHome, this.spots, this.blocked, roll));
    });
  }

  private draw() {
    paint(this.ctx, this.scene());
    this.onAnchor(this.selectedId ? this.anchor(this.selectedId) : null);
  }

  private fit() {
    const scale = Math.max(
      1,
      Math.min(
        Math.floor(this.host.clientWidth / WORLD_W),
        Math.floor(this.host.clientHeight / WORLD_H),
      ),
    );
    this.canvas.style.width = `${WORLD_W * scale}px`;
    this.canvas.style.height = `${WORLD_H * scale}px`;
  }

  private point(event: MouseEvent): { x: number; y: number } | null {
    const rect = this.canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return null;
    return {
      x: ((event.clientX - rect.left) * WORLD_W) / rect.width,
      y: ((event.clientY - rect.top) * WORLD_H) / rect.height,
    };
  }

  private hit(x: number, y: number): "board" | { comm: string } | string | null {
    if (x >= BOARD.x && x <= BOARD.x + BOARD.w && y >= BOARD.y && y <= BOARD.y + BOARD.h) {
      return "board";
    }
    for (const item of this.comms) {
      const spot = commRect(item.id);
      if (!spot) continue;
      if (x >= spot.x && x <= spot.x + spot.w && y >= spot.y && y <= spot.y + spot.h) {
        return { comm: item.id };
      }
    }
    for (const bot of this.bots) {
      const walker = this.walkers.get(bot.id);
      if (!walker) continue;
      if (x >= walker.x && x <= walker.x + 18 && y >= walker.y && y <= walker.y + 26) return bot.id;
    }
    const seats = layout(this.bots.length);
    for (let index = seats.length - 1; index >= 0; index -= 1) {
      const seat = seats[index];
      const box = seatHit(seat);
      if (x >= box.x && x <= box.x + box.w && y >= box.y && y <= box.y + box.h) {
        return this.bots[index]?.id ?? null;
      }
    }
    return null;
  }

  private click(event: MouseEvent) {
    const point = this.point(event);
    if (!point) return;
    const target = this.hit(point.x, point.y);
    if (target === "board") this.onBoard();
    else if (target && typeof target === "object") {
      this.selectedComm = target.comm;
      this.draw();
      this.onComm(target.comm);
    } else if (target) {
      this.selectedComm = null;
      this.onBot(target);
    }
  }

  private hover(event: MouseEvent) {
    const point = this.point(event);
    const target = point ? this.hit(point.x, point.y) : null;
    this.canvas.style.cursor = target ? "pointer" : "default";
  }

  private pause() {
    this.running = false;
    window.clearTimeout(this.timer);
    this.timer = 0;
    this.draw();
  }

  private resume() {
    if (this.reduceMotion || document.hidden) {
      this.running = false;
      this.draw();
      return;
    }
    if (this.timer) return;
    this.running = true;
    const loop = () => {
      this.timer = 0;
      if (!this.running || document.hidden) return;
      this.frame = (this.frame + 1) % 3;
      this.stepWalkers();
      this.draw();
      this.timer = window.setTimeout(loop, 250);
    };
    this.timer = window.setTimeout(loop, 250);
  }
}
