import { invoke } from "@tauri-apps/api/core";

export interface MemoryEntry {
  name: string;
  description: string;
  mem_type: string | null;
  path: string;
}

export interface Skill {
  name: string;
  description: string;
  path: string;
}

export interface AgentDef {
  name: string;
  description: string;
  tools: string | null;
  path: string;
}

export interface SlashCommand {
  name: string;
  description: string | null;
  argument_hint: string | null;
  path: string;
}

export interface Workflow {
  name: string;
  description: string | null;
  when_to_use: string | null;
  path: string;
}

export interface SettingsSummary {
  model: string | null;
  theme: string | null;
  permissions_allow: string[];
  hooks_events: string[];
  enabled_plugins: string[];
}

export interface StagedChange {
  id: string;
  target_path: string;
  draft_content: string;
  created_at: string;
}

export interface AppliedChange {
  id: string;
  target_path: string;
  backup_path: string;
  applied_at: string;
}

/** Forma del error tipado que emite el backend (ver `error.rs`). */
export interface AppErrorShape {
  kind: string;
  message: string;
  path?: string | null;
}

/** Mensaje legible de cualquier error de `invoke()`: usa el `message` del
 *  error estructurado del backend, o cae a `String(err)` si no lo es. */
export function errorMessage(err: unknown): string {
  if (err && typeof err === "object" && "message" in err) {
    return String((err as AppErrorShape).message);
  }
  return String(err);
}

// --- Validación runtime del contrato IPC -----------------------------------
// invoke<T>() confía en el genérico sin verificar la forma real deserializada.
// Los payloads de staging se consumen campo por campo (y un history.jsonl o un
// backup corrupto podría alterar su forma), así que se validan en el borde:
// si el contrato Rust/TS diverge, falla acá con un error claro y no con un
// `undefined` en el punto de uso.

function isStagedChange(x: unknown): x is StagedChange {
  const o = x as Record<string, unknown>;
  return (
    !!o &&
    typeof o.id === "string" &&
    typeof o.target_path === "string" &&
    typeof o.draft_content === "string" &&
    typeof o.created_at === "string"
  );
}

function isAppliedChange(x: unknown): x is AppliedChange {
  const o = x as Record<string, unknown>;
  return (
    !!o &&
    typeof o.id === "string" &&
    typeof o.target_path === "string" &&
    typeof o.backup_path === "string" &&
    typeof o.applied_at === "string"
  );
}

function checkArray<T>(value: unknown, guard: (x: unknown) => x is T, what: string): T[] {
  if (!Array.isArray(value) || !value.every(guard)) {
    throw new Error(`Respuesta inesperada del backend para ${what} — el contrato IPC no coincide.`);
  }
  return value;
}

function check<T>(value: unknown, guard: (x: unknown) => x is T, what: string): T {
  if (!guard(value)) {
    throw new Error(`Respuesta inesperada del backend para ${what} — el contrato IPC no coincide.`);
  }
  return value;
}

export const api = {
  listMemories: () => invoke<MemoryEntry[]>("list_memories"),
  listSkills: () => invoke<Skill[]>("list_skills"),
  listAgents: () => invoke<AgentDef[]>("list_agents"),
  listScheduledTasks: () => invoke<Skill[]>("list_scheduled_tasks"),
  listCommands: () => invoke<SlashCommand[]>("list_commands"),
  listWorkflows: () => invoke<Workflow[]>("list_workflows"),
  readSettingsSummary: () => invoke<SettingsSummary>("read_settings_summary"),
  readClaudeMd: () => invoke<string>("read_claude_md"),
  readFileContent: (path: string) => invoke<string>("read_file_content", { path }),
  settingsPath: () => invoke<string>("settings_path"),

  stageChange: (targetPath: string, draftContent: string) =>
    invoke<unknown>("stage_change", { targetPath, draftContent }).then((r) =>
      check(r, isStagedChange, "stage_change"),
    ),
  listStaged: () =>
    invoke<unknown>("list_staged").then((r) => checkArray(r, isStagedChange, "list_staged")),
  diffStaged: (id: string) => invoke<string>("diff_staged", { id }),
  discardStaged: (id: string) => invoke<void>("discard_staged", { id }),
  applyStaged: (id: string) =>
    invoke<unknown>("apply_staged", { id }).then((r) => check(r, isAppliedChange, "apply_staged")),
  listHistory: () =>
    invoke<unknown>("list_history").then((r) => checkArray(r, isAppliedChange, "list_history")),
  revertApplied: (id: string) =>
    invoke<unknown>("revert_applied", { id }).then((r) =>
      check(r, isAppliedChange, "revert_applied"),
    ),
};
