import type { BoardMetric, Bot, Todo } from "../types";
import { BOARD, layout, paint, seatHit, WORLD_H, WORLD_W, type SceneFrame } from "./draw";

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
  private selectedId: string | null = null;
  private reduceMotion: boolean;

  constructor(
    canvas: HTMLCanvasElement,
    private readonly host: HTMLElement,
    private readonly onBot: (id: string) => void,
    private readonly onBoard: () => void,
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
    this.draw();
  }

  setBoard(todos: Todo[], metrics: BoardMetric[], updated: string | null, focus: string | null) {
    this.todos = todos;
    this.metrics = metrics;
    this.updated = updated;
    this.focus = focus;
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
      selectedId: this.selectedId,
      frame: this.frame,
    };
  }

  private draw() {
    paint(this.ctx, this.scene());
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

  private hit(x: number, y: number): "board" | string | null {
    if (x >= BOARD.x && x <= BOARD.x + BOARD.w && y >= BOARD.y && y <= BOARD.y + BOARD.h) {
      return "board";
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
    else if (target) this.onBot(target);
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
      this.frame ^= 1;
      this.draw();
      this.timer = window.setTimeout(loop, 500);
    };
    this.timer = window.setTimeout(loop, 500);
  }
}
