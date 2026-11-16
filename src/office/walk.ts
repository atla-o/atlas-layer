import { BOARD, WORLD_H, WORLD_W, type Seat } from "./draw";

export const TILE = 16;
export type Dir = "n" | "s" | "e" | "w";
export type Point = { x: number; y: number };

export type Walker = {
  id: string;
  x: number;
  y: number;
  dir: Dir;
  step: number;
  path: Point[];
  wait: number;
  home: Point;
};

const FLOOR_TOP = 80;

export function tileKey(point: Point): string {
  return `${point.x},${point.y}`;
}

export function snap(x: number, y: number): Point {
  return {
    x: Math.round(x / TILE) * TILE,
    y: Math.round(y / TILE) * TILE,
  };
}

export function blockedTiles(seats: Seat[]): Set<string> {
  const blocked = new Set<string>();
  for (const seat of seats) {
    for (let y = seat.y - 8; y < seat.y + seat.h + 6; y += TILE) {
      for (let x = seat.x - 4; x < seat.x + seat.w; x += TILE) {
        blocked.add(tileKey(snap(x, y)));
      }
    }
  }
  return blocked;
}

export function walkable(point: Point, blocked: Set<string>): boolean {
  if (point.x < TILE || point.y < FLOOR_TOP || point.x > WORLD_W - TILE * 2 || point.y > WORLD_H - TILE * 2) {
    return false;
  }
  return !blocked.has(tileKey(point));
}

export function homeTile(seat: Seat, blocked: Set<string>): Point {
  const preferred = snap(seat.x + seat.w / 2, seat.y + seat.h + 10);
  if (walkable(preferred, blocked)) return preferred;
  const fallbacks = [
    snap(seat.x + seat.w / 2, seat.y + seat.h + 26),
    snap(seat.x - 8, seat.y + seat.h + 10),
    snap(seat.x + seat.w + 8, seat.y + seat.h + 10),
  ];
  return fallbacks.find((point) => walkable(point, blocked)) ?? preferred;
}

export function wanderSpots(blocked: Set<string>): Point[] {
  const raw = [
    snap(BOARD.x + 40, BOARD.y + BOARD.h + 28),
    snap(BOARD.x + BOARD.w - 40, BOARD.y + BOARD.h + 28),
    snap(48, 240),
    snap(400, 240),
    snap(240, 160),
  ];
  return raw.filter((point) => walkable(point, blocked));
}

const DIRS: { dir: Dir; dx: number; dy: number }[] = [
  { dir: "e", dx: TILE, dy: 0 },
  { dir: "w", dx: -TILE, dy: 0 },
  { dir: "s", dx: 0, dy: TILE },
  { dir: "n", dx: 0, dy: -TILE },
];

export function findPath(from: Point, to: Point, blocked: Set<string>): Point[] {
  const start = snap(from.x, from.y);
  const goal = snap(to.x, to.y);
  if (tileKey(start) === tileKey(goal)) return [];
  if (!walkable(goal, blocked)) return [];
  const queue: Point[] = [start];
  const prev = new Map<string, string>();
  const seen = new Set<string>([tileKey(start)]);
  while (queue.length) {
    const current = queue.shift()!;
    if (tileKey(current) === tileKey(goal)) break;
    for (const dir of DIRS) {
      const next = { x: current.x + dir.dx, y: current.y + dir.dy };
      const key = tileKey(next);
      if (seen.has(key) || !walkable(next, blocked)) continue;
      seen.add(key);
      prev.set(key, tileKey(current));
      queue.push(next);
    }
  }
  if (!prev.has(tileKey(goal)) && tileKey(start) !== tileKey(goal)) return [];
  const path: Point[] = [];
  let cursor = tileKey(goal);
  while (cursor !== tileKey(start)) {
    const [x, y] = cursor.split(",").map(Number);
    path.push({ x, y });
    const earlier = prev.get(cursor);
    if (!earlier) return [];
    cursor = earlier;
  }
  path.reverse();
  return path;
}

export function createWalker(id: string, home: Point): Walker {
  return { id, x: home.x, y: home.y, dir: "s", step: 0, path: [], wait: 2, home };
}

export function stepWalker(
  walker: Walker,
  goHome: boolean,
  spots: Point[],
  blocked: Set<string>,
  roll: number,
): Walker {
  const next = { ...walker, path: walker.path.slice() };
  if (goHome) {
    const last = next.path[next.path.length - 1];
    if (!last || tileKey(last) !== tileKey(next.home)) {
      next.path = findPath(next, next.home, blocked);
    }
    next.wait = 0;
  } else if (next.path.length === 0) {
    if (next.wait > 0) {
      next.wait -= 1;
      next.step = 0;
      return next;
    }
    const choice = spots[roll % Math.max(spots.length, 1)];
    if (choice) next.path = findPath(next, choice, blocked);
    next.wait = 4 + (roll % 5);
    if (next.path.length === 0) return next;
  }
  const target = next.path[0];
  if (!target) return next;
  const dx = Math.sign(target.x - next.x) * Math.min(8, Math.abs(target.x - next.x));
  const dy = Math.sign(target.y - next.y) * Math.min(8, Math.abs(target.y - next.y));
  if (dx > 0) next.dir = "e";
  else if (dx < 0) next.dir = "w";
  else if (dy > 0) next.dir = "s";
  else if (dy < 0) next.dir = "n";
  next.x += dx;
  next.y += dy;
  next.step = (next.step + 1) % 3;
  if (next.x === target.x && next.y === target.y) next.path.shift();
  return next;
}
