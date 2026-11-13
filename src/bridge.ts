import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import botsFile from "../assets/defaults/bots.json";
import sampleToday from "../examples/today.json";
import sampleTodo from "../examples/todo.json";
import type {
  Bot,
  ChatMessage,
  FeatureFlags,
  OfficeEvent,
  OfficeSnapshot,
  OpenedChat,
  RemoteAgent,
  ServiceReport,
  Todo,
} from "./types";

type Listener = (event: OfficeEvent) => void;

const listeners = new Set<Listener>();

export function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function onOfficeEvent(listener: Listener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function emit(event: OfficeEvent) {
  listeners.forEach((listener) => listener(event));
}

function seedBots(): Bot[] {
  return botsFile.bots.map((bot) => ({
    id: bot.id,
    name: bot.name,
    role: bot.role,
    connector: bot.connector,
    accent: bot.accent,
    hair: bot.hair,
    accessory: bot.accessory,
    model: "model" in bot ? bot.model : null,
    agentId: "agentId" in bot && bot.agentId ? bot.agentId : "",
    agentPinned: Boolean("agentId" in bot && bot.agentId),
  }));
}

function sampleTodos(): { todos: Todo[]; focus: string | null; metrics: OfficeSnapshot["metrics"]; updated: string | null } {
  const todos: Todo[] = sampleTodo.items.map((item) => ({
    id: item.id,
    text: item.text,
    done: item.done,
    today: false,
    source: "todo",
  }));
  const seen = new Set(todos.map((item) => item.id));
  for (const task of sampleToday.tasks) {
    if (seen.has(task.id)) continue;
    todos.push({
      id: task.id,
      text: task.title,
      done: task.completed,
      today: Boolean("today" in task && task.today),
      source: "today",
    });
  }
  todos.sort((left, right) => Number(right.today) - Number(left.today));
  const metrics = "metrics" in sampleToday && Array.isArray(sampleToday.metrics) ? sampleToday.metrics : [];
  const updated = "updated" in sampleToday && typeof sampleToday.updated === "string" ? sampleToday.updated : null;
  return { todos, focus: metrics.length ? null : sampleToday.focus, metrics, updated };
}

type PreviewState = {
  snapshot: OfficeSnapshot;
  history: Record<string, ChatMessage[]>;
  focus: string | null;
};

const previewKey = "atla-office-preview-v2";

const defaultFeatures = (): FeatureFlags => ({
  office: true,
  filter: false,
  sharedCompute: false,
});

function serviceReport(feature: "filter" | "sharedCompute", enabled: boolean): ServiceReport {
  if (feature === "filter") {
    return enabled
      ? {
          feature,
          enabled,
          phase: "standing-by",
          detail: "Armed. The filter engine is not installed yet, so nothing is scanned.",
          running: "Nothing",
          contributed: "0",
          earned: "0",
        }
      : {
          feature,
          enabled,
          phase: "off",
          detail: "Off. Harmful and NSFW filtering is not running.",
          running: "Nothing",
          contributed: "0",
          earned: "0",
        };
  }
  return enabled
    ? {
        feature,
        enabled,
        phase: "idle-wait",
        detail: "On, and waiting. Shared Compute only runs when this machine is idle or plugged in.",
        running: "Nothing",
        contributed: "0",
        earned: "0",
      }
    : {
        feature,
        enabled,
        phase: "off",
        detail: "Off. This machine is not lending compute.",
        running: "Nothing",
        contributed: "0",
        earned: "0",
      };
}

function freshPreview(): PreviewState {
  const board = sampleTodos();
  const statuses: Record<string, string> = {};
  for (const bot of seedBots()) statuses[bot.id] = "idle";
  return {
    snapshot: {
      title: "Atlas Layer",
      bots: seedBots(),
      todos: board.todos,
      todayFocus: board.focus,
      metrics: board.metrics,
      updated: board.updated,
      statuses,
      paths: {
        todo: "examples/todo.json",
        today: "examples/today.json",
        inbox: "~/Atla/AI/office-inbox",
        history: "browser preview",
        config: "browser preview",
        personality: "examples/personality.md",
      },
      warnings: [],
      preview: true,
      features: defaultFeatures(),
      services: {
        filter: serviceReport("filter", false),
        sharedCompute: serviceReport("sharedCompute", false),
      },
    },
    history: {},
    focus: null,
  };
}

function loadPreview(): PreviewState {
  try {
    const raw = localStorage.getItem(previewKey);
    if (!raw) return freshPreview();
    const parsed = JSON.parse(raw) as PreviewState;
    if (!parsed.snapshot?.bots?.length) return freshPreview();
    parsed.snapshot.preview = true;
    parsed.snapshot.features = { ...defaultFeatures(), ...parsed.snapshot.features };
    const flags = parsed.snapshot.features;
    parsed.snapshot.services = {
      filter: serviceReport("filter", flags.filter),
      sharedCompute: serviceReport("sharedCompute", flags.sharedCompute),
    };
    parsed.snapshot.metrics = parsed.snapshot.metrics ?? [];
    parsed.snapshot.updated = parsed.snapshot.updated ?? null;
    parsed.snapshot.todos = parsed.snapshot.todos.map((item) => ({ ...item, today: Boolean(item.today) }));
    return parsed;
  } catch {
    return freshPreview();
  }
}

let preview = loadPreview();

function savePreview() {
  localStorage.setItem(previewKey, JSON.stringify(preview));
}

export async function setFeature(
  feature: keyof FeatureFlags,
  enabled: boolean,
): Promise<OfficeSnapshot> {
  if (!inTauri()) {
    preview.snapshot.features = { ...preview.snapshot.features, [feature]: enabled };
    const flags = preview.snapshot.features;
    preview.snapshot.services = {
      filter: serviceReport("filter", flags.filter),
      sharedCompute: serviceReport("sharedCompute", flags.sharedCompute),
    };
    savePreview();
    return structuredClone(preview.snapshot);
  }
  return invoke<OfficeSnapshot>("set_feature", { feature, enabled });
}

export async function loadOffice(): Promise<OfficeSnapshot> {
  if (!inTauri()) return structuredClone(preview.snapshot);
  return invoke<OfficeSnapshot>("get_office_state");
}

export async function openBot(botId: string): Promise<OpenedChat> {
  if (!inTauri()) {
    preview.focus = botId;
    preview.snapshot.statuses[botId] = "idle";
    savePreview();
    emit({ type: "status", botId, status: "idle" });
    const bot = preview.snapshot.bots.find((item) => item.id === botId);
    return {
      messages: preview.history[botId] ?? [],
      warning:
        bot?.connector === "cursor-agents"
          ? "Browser preview has no Cursor key. The desktop app lists agents and sends follow-ups."
          : null,
      agents:
        bot?.connector === "cursor-agents"
          ? [
              { id: "bc-preview-site", name: "Atlas site", status: "IDLE", url: "" },
              { id: "bc-preview-play", name: "Roleplay", status: "IDLE", url: "" },
            ]
          : [],
      agentId: bot?.agentId ?? "",
    };
  }
  return invoke<OpenedChat>("open_bot", { botId });
}

export async function closeBot(botId: string): Promise<void> {
  if (!inTauri()) {
    if (preview.focus === botId) preview.focus = null;
    savePreview();
    return;
  }
  await invoke("close_bot", { botId });
}

export async function sendMessage(botId: string, text: string): Promise<ChatMessage> {
  if (!inTauri()) return previewSend(botId, text);
  return invoke<ChatMessage>("send_message", { botId, text });
}

export async function addTodo(text: string): Promise<OfficeSnapshot> {
  if (!inTauri()) {
    const item: Todo = {
      id: `m-${crypto.randomUUID()}`,
      text: text.trim(),
      done: false,
      today: false,
      source: "todo",
    };
    preview.snapshot.todos = [...preview.snapshot.todos, item];
    savePreview();
    emit({
      type: "todos",
      todos: preview.snapshot.todos,
      todayFocus: preview.snapshot.todayFocus,
      metrics: preview.snapshot.metrics,
      updated: preview.snapshot.updated,
    });
    return structuredClone(preview.snapshot);
  }
  return invoke<OfficeSnapshot>("add_todo", { text });
}

export async function toggleTodo(id: string): Promise<OfficeSnapshot> {
  if (!inTauri()) {
    preview.snapshot.todos = preview.snapshot.todos.map((item) =>
      item.id === id ? { ...item, done: !item.done, source: item.source === "today" ? "todo" : item.source } : item,
    );
    savePreview();
    emit({
      type: "todos",
      todos: preview.snapshot.todos,
      todayFocus: preview.snapshot.todayFocus,
      metrics: preview.snapshot.metrics,
      updated: preview.snapshot.updated,
    });
    return structuredClone(preview.snapshot);
  }
  return invoke<OfficeSnapshot>("toggle_todo", { id });
}

export async function refreshCursorAgents(): Promise<RemoteAgent[]> {
  if (!inTauri()) {
    return [
      { id: "bc-preview-site", name: "Atlas site", status: "IDLE", url: "" },
      { id: "bc-preview-play", name: "Roleplay", status: "IDLE", url: "" },
    ];
  }
  return invoke<RemoteAgent[]>("refresh_cursor_agents");
}

export async function bindCursorAgent(botId: string, agentId: string): Promise<Bot> {
  if (!inTauri()) {
    const bot = preview.snapshot.bots.find((item) => item.id === botId);
    if (!bot) throw new Error("Unknown desk");
    bot.agentId = agentId;
    bot.agentPinned = true;
    savePreview();
    return { ...bot };
  }
  return invoke<Bot>("bind_cursor_agent", { botId, agentId });
}

export async function watchOffice(): Promise<() => void> {
  if (!inTauri()) return () => undefined;
  return listen<OfficeEvent>("office-event", (event) => emit(event.payload));
}

export async function watchFocus(onFocus: (focused: boolean) => void): Promise<void> {
  if (!inTauri()) {
    window.addEventListener("focus", () => onFocus(true));
    window.addEventListener("blur", () => onFocus(false));
    return;
  }
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  await getCurrentWindow().onFocusChanged(({ payload }) => onFocus(payload));
}

async function previewSend(botId: string, text: string): Promise<ChatMessage> {
  const bot = preview.snapshot.bots.find((item) => item.id === botId);
  if (!bot) throw new Error("Unknown desk");
  const user: ChatMessage = {
    id: `m-${crypto.randomUUID()}`,
    role: "user",
    text,
    ts: new Date().toISOString(),
    read: true,
  };
  const history = preview.history[botId] ?? [];
  history.push(user);
  preview.history[botId] = history;
  preview.snapshot.statuses[botId] = "working";
  savePreview();
  emit({ type: "status", botId, status: "working" });
  const replyId = `${user.id}:reply`;
  const reply =
    bot.connector === "relay-webhook"
      ? `Preview only. The desktop app posts {bot:"${bot.id}", text, reply_to_id:"${user.id}"} to the relay.`
      : bot.connector === "cursor-agents"
        ? "Preview only. The desktop app sends this as a follow-up to the chosen Cursor agent."
        : `Echo mode is on, so this stays in the browser. ${bot.name} heard: ${text}`;
  window.setTimeout(() => streamPreview(botId, replyId, reply), 40);
  return user;
}

function streamPreview(botId: string, messageId: string, reply: string) {
  const parts = reply.split(/(\s+)/);
  let index = 0;
  const step = () => {
    if (index >= parts.length) {
      const message: ChatMessage = {
        id: messageId,
        role: "assistant",
        text: reply,
        ts: new Date().toISOString(),
        read: preview.focus === botId,
      };
      (preview.history[botId] ??= []).push(message);
      const status = preview.focus === botId ? "idle" : "message";
      preview.snapshot.statuses[botId] = status;
      savePreview();
      emit({ type: "done", botId, messageId, text: reply });
      emit({ type: "status", botId, status });
      return;
    }
    emit({ type: "token", botId, messageId, token: parts[index] });
    index += 1;
    window.setTimeout(step, 28);
  };
  step();
}
