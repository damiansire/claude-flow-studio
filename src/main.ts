import { mountEditorModal } from "./lib/editor";
import { renderAgentes } from "./views/agentes";
import { renderAutomatizacion } from "./views/automatizacion";
import { renderComandos } from "./views/comandos";
import { renderHistorial } from "./views/historial";
import { renderMemoria } from "./views/memoria";
import { renderOverview } from "./views/overview";
import { renderReglas } from "./views/reglas";
import { renderSkills } from "./views/skills";

interface Tab {
  id: string;
  label: string;
  icon: string;
  render: (container: HTMLElement) => Promise<void>;
}

const TABS: Tab[] = [
  { id: "overview", label: "Visión general", icon: "◎", render: renderOverview },
  { id: "reglas", label: "Reglas globales", icon: "📜", render: renderReglas },
  { id: "memoria", label: "Memoria", icon: "🧩", render: renderMemoria },
  { id: "skills", label: "Skills", icon: "🛠️", render: renderSkills },
  { id: "comandos", label: "Comandos & Workflows", icon: "⚡", render: renderComandos },
  { id: "agentes", label: "Agentes", icon: "🤖", render: renderAgentes },
  { id: "auto", label: "Automatización", icon: "⏱️", render: renderAutomatizacion },
  { id: "historial", label: "Historial", icon: "🕓", render: renderHistorial },
];

const app = document.querySelector<HTMLDivElement>("#app")!;

app.innerHTML = `
  <nav>
    <h1>🧠 claude-flow-studio</h1>
    <div class="sub">Datos en vivo de ~/.claude</div>
    ${TABS.map((t) => `<button data-tab="${t.id}">${t.icon} ${t.label}</button>`).join("")}
  </nav>
  <main><section class="active" id="section-active"></section></main>
`;

mountEditorModal(app);

const sectionEl = app.querySelector<HTMLElement>("#section-active")!;
const buttons = app.querySelectorAll<HTMLButtonElement>("nav button[data-tab]");

async function activate(tabId: string) {
  buttons.forEach((b) => b.classList.toggle("active", b.dataset.tab === tabId));
  const tab = TABS.find((t) => t.id === tabId)!;
  await tab.render(sectionEl);
}

buttons.forEach((b) => b.addEventListener("click", () => activate(b.dataset.tab!)));

activate(TABS[0].id);
