const GLYPHS: Record<string, string[]> = {
  A: [" # ", "# #", "###", "# #", "# #"],
  B: ["## ", "# #", "## ", "# #", "## "],
  C: [" ##", "#  ", "#  ", "#  ", " ##"],
  D: ["## ", "# #", "# #", "# #", "## "],
  E: ["###", "#  ", "## ", "#  ", "###"],
  F: ["###", "#  ", "## ", "#  ", "#  "],
  G: [" ##", "#  ", "# #", "# #", " ##"],
  H: ["# #", "# #", "###", "# #", "# #"],
  I: ["###", " # ", " # ", " # ", "###"],
  J: [" ##", "  #", "  #", "# #", " # "],
  K: ["# #", "## ", "#  ", "## ", "# #"],
  L: ["#  ", "#  ", "#  ", "#  ", "###"],
  M: ["# #", "###", "# #", "# #", "# #"],
  N: ["# #", "###", "# #", "# #", "# #"],
  O: [" # ", "# #", "# #", "# #", " # "],
  P: ["## ", "# #", "## ", "#  ", "#  "],
  Q: [" # ", "# #", "# #", "## ", " ##"],
  R: ["## ", "# #", "## ", "# #", "# #"],
  S: [" ##", "#  ", " # ", "  #", "## "],
  T: ["###", " # ", " # ", " # ", " # "],
  U: ["# #", "# #", "# #", "# #", " ##"],
  V: ["# #", "# #", "# #", "# #", " # "],
  W: ["# #", "# #", "# #", "###", "# #"],
  X: ["# #", "# #", " # ", "# #", "# #"],
  Y: ["# #", "# #", " # ", " # ", " # "],
  Z: ["###", "  #", " # ", "#  ", "###"],
  "0": [" # ", "# #", "# #", "# #", " # "],
  "1": [" # ", "## ", " # ", " # ", "###"],
  "2": ["## ", "  #", " # ", "#  ", "###"],
  "3": ["## ", "  #", " ##", "  #", "## "],
  "4": ["# #", "# #", "###", "  #", "  #"],
  "5": ["###", "#  ", "## ", "  #", "## "],
  "6": [" ##", "#  ", "## ", "# #", " ##"],
  "7": ["###", "  #", " # ", "#  ", "#  "],
  "8": [" # ", "# #", " # ", "# #", " # "],
  "9": [" ##", "# #", " ##", "  #", "## "],
  " ": ["   ", "   ", "   ", "   ", "   "],
  ".": ["   ", "   ", "   ", "   ", " # "],
  "-": ["   ", "   ", "###", "   ", "   "],
  "'": [" # ", " # ", "   ", "   ", "   "],
  "!": [" # ", " # ", " # ", "   ", " # "],
  "?": ["## ", "  #", " # ", "   ", " # "],
  ":": ["   ", " # ", "   ", " # ", "   "],
};

function glyph(ch: string): string[] {
  return GLYPHS[ch] ?? GLYPHS["?"];
}

export function textWidth(text: string): number {
  const upper = text.toUpperCase();
  let width = 0;
  for (let i = 0; i < upper.length; i += 1) {
    width += glyph(upper[i])[0].length + 1;
  }
  return Math.max(0, width - 1);
}

export function fitText(text: string, maxWidth: number): string {
  const upper = text.toUpperCase();
  if (textWidth(upper) <= maxWidth) return upper;
  let cut = upper;
  while (cut.length > 1 && textWidth(cut) > maxWidth) {
    cut = cut.slice(0, -1);
  }
  return cut;
}

export function drawText(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  color: string,
): number {
  ctx.fillStyle = color;
  let cursor = x;
  const upper = text.toUpperCase();
  for (let i = 0; i < upper.length; i += 1) {
    const rows = glyph(upper[i]);
    rows.forEach((row, ry) => {
      for (let rx = 0; rx < row.length; rx += 1) {
        if (row[rx] === "#") ctx.fillRect(cursor + rx, y + ry, 1, 1);
      }
    });
    cursor += rows[0].length + 1;
  }
  return cursor - x - 1;
}
