import http from "node:http";

const port = Number(process.env.ATLA_MOCK_OLLAMA_PORT ?? 11439);
const server = http.createServer((req, res) => {
  if (req.method === "GET" && (req.url === "/api/tags" || req.url === "/")) {
    res.writeHead(200, { "content-type": "application/json" });
    res.end(JSON.stringify({ models: [{ name: "qwen3:8b" }] }));
    return;
  }
  if (req.method !== "POST" || req.url !== "/api/chat") {
    res.writeHead(404);
    res.end();
    return;
  }
  const chunks = [];
  req.on("data", (chunk) => chunks.push(chunk));
  req.on("end", () => {
    let heard = "";
    try {
      const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      const messages = Array.isArray(body.messages) ? body.messages : [];
      const last = [...messages].reverse().find((message) => message.role === "user");
      heard = String(last?.content ?? "").slice(0, 180);
    } catch {
      heard = "";
    }
    const words = `Local Atlas, on this machine: ${heard || "I am here."}`.split(/(\s+)/);
    res.writeHead(200, { "content-type": "application/x-ndjson" });
    let index = 0;
    const timer = setInterval(() => {
      if (index >= words.length) {
        res.write(`${JSON.stringify({ message: { role: "assistant", content: "" }, done: true })}\n`);
        res.end();
        clearInterval(timer);
        return;
      }
      res.write(
        `${JSON.stringify({ message: { role: "assistant", content: words[index] }, done: false })}\n`,
      );
      index += 1;
    }, 35);
  });
});

server.listen(port, "127.0.0.1", () => {
  console.log(`[atlas-layer] mock ollama on http://127.0.0.1:${port}`);
});
