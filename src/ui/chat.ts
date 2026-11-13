import { bindCursorAgent, closeBot, openBot, refreshCursorAgents, sendMessage } from "../bridge";
import type { Bot, ChatMessage, OfficeEvent, RemoteAgent } from "../types";

const root = () => document.querySelector<HTMLElement>("#chat")!;

export class ChatPanel {
  private bot: Bot | null = null;
  private messages: ChatMessage[] = [];
  private agents: RemoteAgent[] = [];
  private sending = false;

  constructor(private readonly onStatus: (id: string, status: string) => void) {
    document.querySelector<HTMLButtonElement>("#chat-close")!.addEventListener("click", () => {
      void this.close();
    });
    document.querySelector<HTMLFormElement>("#chat-form")!.addEventListener("submit", (event) => {
      event.preventDefault();
      void this.submit();
    });
    const input = document.querySelector<HTMLTextAreaElement>("#chat-input")!;
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        void this.submit();
      }
    });
    document.querySelector<HTMLButtonElement>("#agent-pin")!.addEventListener("click", () => {
      void this.pin();
    });
    document.querySelector<HTMLButtonElement>("#agent-refresh")!.addEventListener("click", () => {
      void this.refresh();
    });
  }

  async open(bot: Bot) {
    this.bot = bot;
    root().hidden = false;
    this.renderHead();
    this.setWarning("");
    this.messages = [];
    this.renderLog();
    try {
      const opened = await openBot(bot.id);
      if (this.bot?.id !== bot.id) return;
      this.messages = opened.messages;
      this.agents = opened.agents;
      if (opened.agentId) this.bot = { ...bot, agentId: opened.agentId };
      this.renderHead();
      this.renderAgents();
      this.renderLog();
      this.setWarning(opened.warning ?? "");
    } catch (error) {
      this.setWarning(error instanceof Error ? error.message : "The desk did not open.");
    }
    document.querySelector<HTMLTextAreaElement>("#chat-input")!.focus();
  }

  async close() {
    const id = this.bot?.id;
    root().hidden = true;
    this.bot = null;
    if (id) await closeBot(id);
  }

  get openId(): string | null {
    return this.bot?.id ?? null;
  }

  apply(event: OfficeEvent) {
    if (!this.bot) return;
    const raw = event as unknown as { botId?: string; bot_id?: string };
    const botId = raw.botId ?? raw.bot_id ?? null;
    if (botId !== this.bot.id) return;
    if (event.type === "token") {
      const existing = this.messages.find((message) => message.id === event.messageId);
      if (existing) existing.text += event.token;
      else {
        this.messages.push({
          id: event.messageId,
          role: "assistant",
          text: event.token,
          ts: new Date().toISOString(),
        });
      }
      this.renderLog();
    } else if (event.type === "done") {
      const existing = this.messages.find((message) => message.id === event.messageId);
      if (existing) existing.text = event.text;
      else {
        this.messages.push({
          id: event.messageId,
          role: "assistant",
          text: event.text,
          ts: new Date().toISOString(),
        });
      }
      this.renderLog();
      this.wait(false);
    } else if (event.type === "message") {
      if (!this.messages.some((message) => message.id === event.message.id)) {
        this.messages.push(event.message);
      }
      this.renderLog();
      this.wait(false);
    } else if (event.type === "error") {
      this.messages.push({
        id: `err-${event.messageId}`,
        role: "system",
        text: event.message,
        ts: new Date().toISOString(),
      });
      this.renderLog();
      this.wait(false);
    } else if (event.type === "accepted") {
      this.wait(true);
    }
  }

  private async submit() {
    if (!this.bot || this.sending) return;
    const input = document.querySelector<HTMLTextAreaElement>("#chat-input")!;
    const text = input.value.trim();
    if (!text) return;
    this.sending = true;
    input.value = "";
    this.setWarning("");
    try {
      const user = await sendMessage(this.bot.id, text);
      if (!this.messages.some((message) => message.id === user.id)) this.messages.push(user);
      this.renderLog();
      this.onStatus(this.bot.id, "working");
      if (this.bot.connector === "relay-webhook" || this.bot.connector === "local-window") this.wait(true);
    } catch (error) {
      this.setWarning(error instanceof Error ? error.message : "The message did not send.");
      input.value = text;
    } finally {
      this.sending = false;
    }
  }

  private async pin() {
    if (!this.bot) return;
    const select = document.querySelector<HTMLSelectElement>("#agent-select")!;
    const agentId = select.value;
    if (!agentId) return;
    try {
      this.bot = await bindCursorAgent(this.bot.id, agentId);
      this.renderHead();
      this.setWarning("");
    } catch (error) {
      this.setWarning(error instanceof Error ? error.message : "Could not save that agent.");
    }
  }

  private async refresh() {
    try {
      this.agents = await refreshCursorAgents();
      this.renderAgents();
    } catch (error) {
      this.setWarning(error instanceof Error ? error.message : "Could not list agents.");
    }
  }

  private renderHead() {
    const bot = this.bot;
    if (!bot) return;
    document.querySelector("#chat-name")!.textContent = bot.name;
    document.querySelector("#chat-role")!.textContent = bot.role;
    const swatch = document.querySelector<HTMLElement>("#chat-swatch")!;
    swatch.style.background = bot.accent;
    const meta = document.querySelector("#chat-meta")!;
    const kind =
      bot.connector === "local-ollama"
        ? `Local model${bot.model ? ` · ${bot.model}` : ""}`
        : bot.connector === "local-window"
          ? "Local window"
          : bot.connector === "relay-webhook"
          ? "Cloud relay"
          : bot.connector === "cursor-agents"
            ? bot.agentId
              ? `Cursor agent · ${bot.agentId}`
              : "Cursor agent"
            : "Echo";
    meta.textContent = kind;
    const row = document.querySelector<HTMLElement>("#agent-row")!;
    row.hidden = bot.connector !== "cursor-agents";
  }

  private renderAgents() {
    const select = document.querySelector<HTMLSelectElement>("#agent-select")!;
    select.replaceChildren();
    const blank = document.createElement("option");
    blank.value = "";
    blank.textContent = this.agents.length ? "Choose an agent" : "No agents loaded";
    select.append(blank);
    for (const agent of this.agents) {
      const option = document.createElement("option");
      option.value = agent.id;
      option.textContent = `${agent.name} · ${agent.status.toLowerCase()}`;
      if (agent.id === this.bot?.agentId) option.selected = true;
      select.append(option);
    }
  }

  private renderLog() {
    const log = document.querySelector<HTMLElement>("#chat-log")!;
    log.replaceChildren();
    if (this.messages.length === 0) {
      const empty = document.createElement("p");
      empty.className = "empty";
      empty.textContent = this.bot ? `Pull up a chair. Say hello to ${this.bot.name}.` : "";
      log.append(empty);
      return;
    }
    for (const { message, depth } of thread(this.messages)) {
      const bubble = document.createElement("article");
      bubble.className = depth > 0 ? `bubble thread ${message.role}` : `bubble ${message.role}`;
      bubble.textContent = message.text;
      const time = document.createElement("time");
      time.dateTime = message.ts;
      time.textContent = clock(message.ts);
      bubble.append(time);
      log.append(bubble);
    }
    log.scrollTop = log.scrollHeight;
    const last = this.messages[this.messages.length - 1];
    const waits =
      this.bot?.connector === "relay-webhook" || this.bot?.connector === "local-window";
    this.wait(Boolean(last && last.role === "user" && waits));
  }

  private wait(show: boolean) {
    document.querySelector<HTMLElement>("#chat-wait")!.hidden = !show;
  }

  private setWarning(text: string) {
    const node = document.querySelector<HTMLElement>("#chat-warning")!;
    node.hidden = !text;
    node.textContent = text;
  }
}

function thread(messages: ChatMessage[]): { message: ChatMessage; depth: number }[] {
  const ids = new Set(messages.map((message) => message.id));
  const children = new Map<string, ChatMessage[]>();
  const roots: ChatMessage[] = [];
  for (const message of messages) {
    const parent = message.inReplyTo;
    if (parent && parent !== message.id && ids.has(parent)) {
      const list = children.get(parent) ?? [];
      list.push(message);
      children.set(parent, list);
    } else {
      roots.push(message);
    }
  }
  const out: { message: ChatMessage; depth: number }[] = [];
  const seen = new Set<string>();
  const walk = (message: ChatMessage, depth: number) => {
    if (seen.has(message.id)) return;
    seen.add(message.id);
    out.push({ message, depth });
    for (const child of children.get(message.id) ?? []) walk(child, Math.min(depth + 1, 4));
  };
  for (const root of roots) walk(root, 0);
  return out;
}

function clock(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}
