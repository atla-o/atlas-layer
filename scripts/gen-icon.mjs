import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function crc32(buf) {
  let c = ~0;
  for (let i = 0; i < buf.length; i += 1) {
    c ^= buf[i];
    for (let k = 0; k < 8; k += 1) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const name = Buffer.from(type);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([name, data])));
  return Buffer.concat([len, name, data, crc]);
}

function png(size, pixels) {
  const raw = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y += 1) {
    const row = y * (size * 4 + 1);
    raw[row] = 0;
    for (let x = 0; x < size; x += 1) {
      const [r, g, b, a] = pixels(x, y);
      const i = row + 1 + x * 4;
      raw[i] = r;
      raw[i + 1] = g;
      raw[i + 2] = b;
      raw[i + 3] = a;
    }
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8;
  ihdr[9] = 6;
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", ihdr),
    chunk("IDAT", zlib.deflateSync(raw)),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function icon(size) {
  return png(size, (x, y) => {
    const cx = size / 2;
    const cy = size / 2;
    const nx = (x + 0.5 - cx) / (size / 2);
    const ny = (y + 0.5 - cy) / (size / 2);
    const d = Math.hypot(nx, ny);
    const ink = [36, 28, 23, 255];
    const terra = [196, 92, 62, 255];
    const cream = [247, 243, 234, 255];
    const wall = [234, 220, 200, 255];
    if (d > 0.92) return [0, 0, 0, 0];
    if (d > 0.84) return ink;
    if (d > 0.62) return terra;
    if (d > 0.4) return cream;
    if (d > 0.28) return wall;
    return terra;
  });
}

const outDir = path.join(root, "packaging");
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, "icon-128.png"), icon(128));
fs.writeFileSync(path.join(outDir, "icon-48.png"), icon(48));
console.log("wrote packaging/icon-128.png");
