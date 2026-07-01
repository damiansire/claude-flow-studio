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
    invoke<StagedChange>("stage_change", { targetPath, draftContent }),
  listStaged: () => invoke<StagedChange[]>("list_staged"),
  diffStaged: (id: string) => invoke<string>("diff_staged", { id }),
  discardStaged: (id: string) => invoke<void>("discard_staged", { id }),
  applyStaged: (id: string) => invoke<AppliedChange>("apply_staged", { id }),
  listHistory: () => invoke<AppliedChange[]>("list_history"),
  revertApplied: (id: string) => invoke<AppliedChange>("revert_applied", { id }),
};
