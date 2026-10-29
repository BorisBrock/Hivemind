import { commands, type Project, type Settings, type TicketStatus } from "./bindings";
import { applyAppearance } from "./theme";

/** Ticket as returned by the backend. */
export type Ticket = Awaited<ReturnType<typeof commands.listTickets>>[number];

export const COLUMNS: { status: TicketStatus; label: string }[] = [
  { status: "planning", label: "Planning" },
  { status: "ready", label: "Ready" },
  { status: "in_progress", label: "In Progress" },
  { status: "needs_input", label: "Needs Input" },
  { status: "done", label: "Done" },
];

/** Global app state, loaded from the backend on start-up. */
class AppState {
  projects = $state<Project[]>([]);
  tickets = $state<Ticket[]>([]);
  settings = $state<Settings>({ theme: "midnight", palette: "vivid", agent: "claude" });
  error = $state<string | null>(null);

  projectsById = $derived(new Map(this.projects.map((p) => [p.id, p])));

  /** Tickets of visible projects, grouped by column, in board order. */
  columns = $derived.by(() => {
    const byStatus = new Map<TicketStatus, Ticket[]>(COLUMNS.map((c) => [c.status, []]));
    for (const t of this.tickets) {
      if (!this.projectsById.get(t.projectId)?.hidden) byStatus.get(t.status)?.push(t);
    }
    for (const list of byStatus.values()) list.sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
    return byStatus;
  });

  async load() {
    await this.run(async () => {
      [this.settings, this.projects, this.tickets] = await Promise.all([
        commands.getSettings(),
        commands.listProjects(),
        commands.listTickets(),
      ]);
      applyAppearance(this.settings.theme, this.settings.palette);
    });
  }

  async updateSettings(patch: Partial<Settings>) {
    const next = { ...this.settings, ...patch };
    applyAppearance(next.theme, next.palette);
    this.settings = next;
    await this.run(() => commands.saveSettings(next));
  }

  async toggleProjectHidden(project: Project) {
    await this.run(async () => {
      const updated = await commands.updateProject(project.id, { hidden: !project.hidden });
      this.projects = this.projects.map((p) => (p.id === updated.id ? updated : p));
    });
  }

  /** Development helper until project/ticket editing lands in Phase 2. */
  async seedDemoData() {
    await this.run(async () => {
      const demo = [
        { name: "Workzone", path: "~/git/workzone" },
        { name: "Website", path: "~/git/website" },
        { name: "Data Pipeline", path: "~/git/pipeline" },
      ];
      const titles = ["Add dark mode toggle", "Fix flaky login test", "Write README"];
      for (const [i, d] of demo.entries()) {
        const p = await commands.createProject(d);
        for (const title of titles.slice(0, titles.length - i)) {
          const t = await commands.createTicket({ projectId: p.id, title, body: `# ${title}\n\nDetails…` });
          if (title.startsWith("Fix")) await commands.moveTicket(t.id, "ready", 0);
        }
      }
      await this.load();
    });
  }

  private async run(fn: () => Promise<unknown>) {
    try {
      this.error = null;
      await fn();
    } catch (e) {
      this.error = typeof e === "object" && e && "kind" in e ? JSON.stringify(e) : String(e);
    }
  }
}

export const app = new AppState();
