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
    <div class="tablist" role="tablist" aria-label="Secciones" aria-orientation="vertical">
      ${TABS.map(
        (t) =>
          `<button role="tab" id="tab-${t.id}" data-tab="${t.id}" aria-controls="section-active" aria-selected="false" tabindex="-1">${t.icon} ${t.label}</button>`,
      ).join("")}
    </div>
  </nav>
  <main><section class="active" id="section-active" role="tabpanel" tabindex="0"></section></main>
`;

mountEditorModal(app);

const sectionEl = app.querySelector<HTMLElement>("#section-active")!;
const tablist = app.querySelector<HTMLElement>('[role="tablist"]')!;
const buttons = Array.from(app.querySelectorAll<HTMLButtonElement>('button[role="tab"]'));

let currentTab = TABS[0].id;

async function activate(tabId: string, focusTab = false) {
  currentTab = tabId;
  buttons.forEach((b) => {
    const selected = b.dataset.tab === tabId;
    b.classList.toggle("active", selected);
    b.setAttribute("aria-selected", String(selected));
    // Roving tabindex: solo el tab activo es tabulable; entre tabs se navega
    // con flechas (patrón APG de tablist).
    b.tabIndex = selected ? 0 : -1;
    if (selected && focusTab) b.focus();
  });
  sectionEl.setAttribute("aria-labelledby", `tab-${tabId}`);
  const tab = TABS.find((t) => t.id === tabId)!;
  await tab.render(sectionEl);
}

buttons.forEach((b) => b.addEventListener("click", () => activate(b.dataset.tab!)));

tablist.addEventListener("keydown", (e) => {
  const step = e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : e.key === "ArrowUp" || e.key === "ArrowLeft" ? -1 : 0;
  if (step === 0) return;
  e.preventDefault();
  const idx = buttons.findIndex((b) => b.dataset.tab === currentTab);
  const next = buttons[(idx + step + buttons.length) % buttons.length];
  activate(next.dataset.tab!, true);
});

// La vista activa se refresca cuando el editor aplica/descarta un cambio, para
// no quedar mostrando datos viejos tras tocar un archivo real.
document.addEventListener("cf:mutated", () => activate(currentTab));

activate(currentTab);
