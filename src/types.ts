export type ConnectorKind = "local-ollama" | "local-window" | "relay-webhook" | "cursor-agents" | "echo";

export type Bot = {
  id: string;
  name: string;
  role: string;
  connector: ConnectorKind | string;
  accent: string;
  hair: string;
  accessory: string;
  model?: string | null;
  agentId: string;
  agentPinned: boolean;
};

export type Todo = {
  id: string;
  text: string;
  done: boolean;
  today: boolean;
  source: "todo" | "today" | string;
};

export type BoardMetric = {
  label: string;
  value: string;
  bad: boolean;
};

export type ChatMessage = {
  id: string;
  role: "user" | "assistant" | "system" | string;
  text: string;
  ts: string;
  read?: boolean;
  inReplyTo?: string | null;
};

export type Bill = {
  id: string;
  name: string;
  amount: string;
  due: string;
  paid: boolean;
};

export type WaitingReply = {
  id: string;
  from: string;
  text: string;
  since: string;
};

export type Decision = {
  id: string;
  title: string;
  detail: string;
  due: string;
};

export type OfficePaths = {
  todo: string;
  today: string;
  dashboard: string;
  inbox: string;
  history: string;
  config: string;
  personality: string;
};

export type FeatureFlags = {
  office: boolean;
  filter: boolean;
  sharedCompute: boolean;
};

export type ServiceReport = {
  feature: string;
  enabled: boolean;
  phase: string;
  detail: string;
  running: string;
  contributed: string;
  earned: string;
};

export type CommObject = {
  id: string;
  label: string;
  mode: "window" | "local" | string;
};

export type OfficeSnapshot = {
  title: string;
  bots: Bot[];
  todos: Todo[];
  todayFocus: string | null;
  metrics: BoardMetric[];
  updated: string | null;
  bills: Bill[];
  replies: WaitingReply[];
  decisions: Decision[];
  dashboardUpdated: string | null;
  statuses: Record<string, string>;
  paths: OfficePaths;
  warnings: string[];
  preview: boolean;
  features: FeatureFlags;
  services: {
    filter: ServiceReport;
    sharedCompute: ServiceReport;
  };
  comms: CommObject[];
};

export type RemoteAgent = {
  id: string;
  name: string;
  status: string;
  url: string;
};

export type OpenedChat = {
  messages: ChatMessage[];
  warning: string | null;
  agents: RemoteAgent[];
  agentId: string;
};

export type OfficeEvent =
  | { type: "token"; botId: string; messageId: string; token: string }
  | { type: "done"; botId: string; messageId: string; text: string }
  | { type: "error"; botId: string; messageId: string; message: string }
  | { type: "accepted"; botId: string; messageId: string }
  | { type: "status"; botId: string; status: string }
  | { type: "todos"; todos: Todo[]; todayFocus: string | null; metrics: BoardMetric[]; updated: string | null }
  | { type: "home"; bills: Bill[]; replies: WaitingReply[]; decisions: Decision[]; updated: string | null }
  | { type: "message"; botId: string; message: ChatMessage }
  | { type: "warning"; message: string }
  | { type: "badge"; id: string; unread: number };
