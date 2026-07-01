//! Integración: arma un `~/.claude` de juguete (sin mocks de filesystem, un
//! tempdir real) y corre las funciones de escaneo contra él.

use cf_core::scan;
use std::fs;
use tempfile::tempdir;

fn fake_claude_dir() -> tempfile::TempDir {
    let root = tempdir().unwrap();
    let base = root.path();

    let memory_dir = base.join("projects").join("C--Users-tester").join("memory");
    fs::create_dir_all(&memory_dir).unwrap();
    fs::write(memory_dir.join("MEMORY.md"), "# Memory index\n").unwrap();
    fs::write(
        memory_dir.join("ejemplo.md"),
        "---\nname: ejemplo\ndescription: \"Una memoria de prueba\"\nmetadata:\n  type: project\n---\nCuerpo.\n",
    )
    .unwrap();

    let skill_dir = base.join("skills").join("mi-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: mi-skill\ndescription: Hace cosas\n---\n# Mi skill\n",
    )
    .unwrap();

    let agents_dir = base.join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("design-reviewer.md"),
        "---\nname: design-reviewer\ndescription: >-\n  Revisa el diseño de una pantalla.\n  Es read-only.\ntools: Bash, Read, Grep, Glob\n---\nBody\n",
    )
    .unwrap();

    let scheduled_dir = base.join("scheduled-tasks").join("mi-tarea");
    fs::create_dir_all(&scheduled_dir).unwrap();
    fs::write(
        scheduled_dir.join("SKILL.md"),
        "---\nname: mi-tarea\ndescription: Recordatorio de prueba\n---\nBody\n",
    )
    .unwrap();

    let commands_dir = base.join("commands");
    fs::create_dir_all(&commands_dir).unwrap();
    fs::write(
        commands_dir.join("audit.md"),
        "---\ndescription: Corre el audit\nargument-hint: [repo]\n---\nContenido.\n",
    )
    .unwrap();

    let workflows_dir = base.join("workflows");
    fs::create_dir_all(&workflows_dir).unwrap();
    fs::write(
        workflows_dir.join("mi-workflow.js"),
        "export const meta = {\n  name: 'mi-workflow',\n  description: 'Hace un review',\n}\n",
    )
    .unwrap();

    fs::write(
        base.join("settings.json"),
        r#"{
          "model": "sonnet",
          "theme": "dark",
          "permissions": { "allow": ["Bash(git status)", "Bash(npm test *)"] },
          "hooks": { "SessionEnd": [] },
          "enabledPlugins": { "rust-analyzer-lsp@claude-plugins-official": true }
        }"#,
    )
    .unwrap();

    root
}

#[test]
fn lists_memories_skipping_the_index() {
    let claude_dir = fake_claude_dir();
    let memories = scan::list_memories(claude_dir.path()).unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].name, "ejemplo");
    assert_eq!(memories[0].description, "Una memoria de prueba");
    assert_eq!(memories[0].mem_type.as_deref(), Some("project"));
}

#[test]
fn lists_skills_from_subfolders() {
    let claude_dir = fake_claude_dir();
    let skills = scan::list_skills(claude_dir.path()).unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "mi-skill");
    assert_eq!(skills[0].description, "Hace cosas");
}

#[test]
fn lists_agents_with_folded_description_and_tools() {
    let claude_dir = fake_claude_dir();
    let agents = scan::list_agents(claude_dir.path()).unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].name, "design-reviewer");
    assert_eq!(
        agents[0].description,
        "Revisa el diseño de una pantalla. Es read-only."
    );
    assert_eq!(agents[0].tools.as_deref(), Some("Bash, Read, Grep, Glob"));
}

#[test]
fn lists_scheduled_tasks_from_subfolders() {
    let claude_dir = fake_claude_dir();
    let tasks = scan::list_scheduled_tasks(claude_dir.path()).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "mi-tarea");
    assert_eq!(tasks[0].description, "Recordatorio de prueba");
}

#[test]
fn lists_commands_with_argument_hint() {
    let claude_dir = fake_claude_dir();
    let commands = scan::list_commands(claude_dir.path()).unwrap();
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].name, "audit");
    assert_eq!(commands[0].argument_hint.as_deref(), Some("[repo]"));
}

#[test]
fn lists_workflows_parsing_the_meta_block() {
    let claude_dir = fake_claude_dir();
    let workflows = scan::list_workflows(claude_dir.path()).unwrap();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].name, "mi-workflow");
    assert_eq!(workflows[0].description.as_deref(), Some("Hace un review"));
}

#[test]
fn reads_settings_summary() {
    let claude_dir = fake_claude_dir();
    let summary = scan::read_settings_summary(claude_dir.path()).unwrap();
    assert_eq!(summary.model.as_deref(), Some("sonnet"));
    assert_eq!(summary.theme.as_deref(), Some("dark"));
    assert_eq!(summary.permissions_allow.len(), 2);
    assert_eq!(summary.hooks_events, vec!["SessionEnd".to_string()]);
    assert_eq!(
        summary.enabled_plugins,
        vec!["rust-analyzer-lsp@claude-plugins-official".to_string()]
    );
}

#[test]
fn missing_directories_return_empty_lists_not_errors() {
    let root = tempdir().unwrap();
    assert!(scan::list_memories(root.path()).unwrap().is_empty());
    assert!(scan::list_skills(root.path()).unwrap().is_empty());
    assert!(scan::list_commands(root.path()).unwrap().is_empty());
    assert!(scan::list_workflows(root.path()).unwrap().is_empty());
}
