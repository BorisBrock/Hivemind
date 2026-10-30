import {
  commands,
  events,
  type AppError,
  type GroupPatch,
  type NewProject,
  type NewTicket,
  type ProjectGroup,
  type ProjectPatch,
  type ProjectPlacement,
  type Project_Serialize as Project,
  type Settings_Serialize as Settings,
  type Usage,
  type TicketPatch,
  type TicketStatus,
} from "./bindings";
import { notifyAgentStopped } from "./notify";
import { applyAppearance } from "./theme";

export type { Project, ProjectGroup };

/** Ticket as returned by the backend. */
export type Ticket = Awaited<ReturnType<typeof commands.listTickets>>[number];
/** One transcript entry as returned by the backend. */
export type TicketEvent = Awaited<ReturnType<typeof commands.listTicketEvents>>[number];

export const COLUMNS: { status: TicketStatus; label: string }[] = [
  { status: "planning", label: "Planning" },
  { status: "ready", label: "Ready" },
  { status: "in_progress", label: "In Progress" },
  { status: "needs_input", label: "Needs Input" },
  { status: "done", label: "Done" },
];

/** Mirrors `TicketStatus::user_can_move_to` in the backend. */
export function userCanMove(from: TicketStatus, to: TicketStatus) {
  return from === to || (from === "planning" && to === "ready") || (from === "ready" && to === "planning");
}

export function errorMessage(e: unknown): string {
  if (typeof e === "object" && e && "kind" in e) {
    const err = e as AppError;
    switch (err.kind) {
      case "database":
      case "invalid":
        return err.message;
      case "not_found":
        return "Not found.";
      case "not_editable":
        return "This ticket can only be edited while it is in Planning.";
      case "invalid_transition":
        return `A ticket can't be moved from ${err.from} to ${err.to}.`;
    }
  }
  return String(e);
}

export type ProjectDialogState = { mode: "create"; groupId?: number } | { mode: "edit"; project: Project };
export type GroupDialogState = { mode: "create" } | { mode: "edit"; group: ProjectGroup };
export type TicketDialogState = { mode: "create"; projectId?: number } | { mode: "view"; ticketId: number };

/** Global app state, loaded from the backend on start-up. */
class AppState {
  projects = $state<Project[]>([]);
  /** Project groups in sidebar order. */
  groups = $state<ProjectGroup[]>([]);
  tickets = $state<Ticket[]>([]);
  settings = $state<Settings>({
    theme: "midnight",
    palette: "vivid",
    agent: "claude",
    claudePath: "",
    opencodePath: "",
    claudeModel: "",
    opencodeModel: "",
    ungroupedProjects: "top",
  });
  usage = $state<Usage | null>(null);
  /** Checked-out git branch per project id; null for folders that aren't git repositories. */
  branches = $state<Record<number, string | null>>({});
  /** Whether branches can be clicked to open the repository in GitHub Desktop. */
  hasGithubDesktop = $state(false);
  settingsOpen = $state(false);
  error = $state<string | null>(null);

  projectDialog = $state<ProjectDialogState | null>(null);
  groupDialog = $state<GroupDialogState | null>(null);
  ticketDialog = $state<TicketDialogState | null>(null);
  /** Project of the last ticket created this session; preselected for the next new ticket. */
  lastTicketProjectId: number | undefined = undefined;
  /** Ticket currently being dragged on the board. */
  dragging = $state<{ id: number; from: TicketStatus } | null>(null);

  /** Transcripts of tickets opened in this session, kept live by backend events. */
  transcripts = $state<Record<number, TicketEvent[]>>({});

  /** Board search; matches title, prompt and project name. */
  search = $state("");

  projectsById = $derived(new Map(this.projects.map((p) => [p.id, p])));
  runningCount = $derived(this.tickets.filter((t) => t.status === "in_progress").length);

  /** Tickets of visible projects, grouped by column, in board order. */
  columns = $derived.by(() => {
    const byStatus = new Map<TicketStatus, Ticket[]>(COLUMNS.map((c) => [c.status, []]));
    const words = this.search.toLowerCase().split(/\s+/).filter(Boolean);
    for (const t of this.tickets) {
      const project = this.projectsById.get(t.projectId);
      if (project?.hidden) continue;
      const text = `${t.title}\n${t.body}\n${project?.name ?? ""}`.toLowerCase();
      if (words.every((w) => text.includes(w))) byStatus.get(t.status)?.push(t);
    }
    for (const list of byStatus.values()) list.sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
    return byStatus;
  });

  private listening = false;

  async load() {
    if (!this.listening) {
      this.listening = true;
      await this.run(() => this.listen());
    }
    await this.run(async () => {
      [this.settings, this.projects, this.groups, this.tickets, this.usage] = await Promise.all([
        commands.getSettings(),
        commands.listProjects(),
        commands.listGroups(),
        commands.listTickets(),
        commands.getUsage(),
      ]);
      applyAppearance(this.settings.theme, this.settings.palette);
    });
    await this.refreshBranches();
    this.hasGithubDesktop = await commands.hasGithubDesktop().catch(() => false);
  }

  /** Re-reads the git branches; they change outside the app, e.g. by agents or the user. */
  async refreshBranches(projects: Project[] = this.projects) {
    const found = await Promise.all(projects.map((p) => commands.gitBranch(p.path).catch(() => null)));
    projects.forEach((p, i) => (this.branches[p.id] = found[i]));
  }

  async updateSettings(patch: Partial<Settings>) {
    const next = { ...this.settings, ...patch };
    applyAppearance(next.theme, next.palette);
    this.settings = next;
    await this.run(() => commands.saveSettings(next));
  }

  /** Agent-driven changes arrive as events from the backend. */
  private async listen() {
    await events.ticketChanged.listen(({ payload }) => {
      const before = this.tickets.find((t) => t.id === payload.id);
      if (
        before?.status === "in_progress" &&
        (payload.status === "needs_input" || payload.status === "done")
      ) {
        const project = this.projectsById.get(payload.projectId)?.name ?? "";
        notifyAgentStopped(payload, project).catch(() => {}); // notifications are best effort
      }
      if (before?.status === "in_progress" && payload.status !== "in_progress") {
        const project = this.projectsById.get(payload.projectId);
        if (project) this.refreshBranches([project]); // the agent may have switched branches
      }
      const exists = this.tickets.some((t) => t.id === payload.id);
      this.tickets = exists
        ? this.tickets.map((t) => (t.id === payload.id ? payload : t))
        : [...this.tickets, payload];
    });
    await events.ticketEventAdded.listen(({ payload }) => {
      const list = this.transcripts[payload.ticketId];
      if (list && !list.some((e) => e.id === payload.id)) list.push(payload);
    });
    await events.usageChanged.listen(({ payload }) => (this.usage = payload));
    window.addEventListener("focus", () => this.refreshBranches());
  }

  // ---- projects -------------------------------------------------------------

  async createProject(project: NewProject) {
    const created = await this.run(() => commands.createProject(project));
    if (created) {
      this.projects = [...this.projects, created];
      await this.refreshBranches([created]);
    }
    return created;
  }

  async updateProject(id: number, patch: ProjectPatch) {
    const updated = await this.run(() => commands.updateProject(id, patch));
    if (updated) {
      this.projects = this.projects.map((p) => (p.id === id ? updated : p));
      if (patch.path != null) await this.refreshBranches([updated]);
    }
    return updated;
  }

  openFolder(path: string) {
    return this.run(() => commands.openFolder(path));
  }

  openInGithubDesktop(path: string) {
    return this.run(() => commands.openInGithubDesktop(path));
  }

  toggleProjectHidden(project: Project) {
    return this.updateProject(project.id, { hidden: !project.hidden });
  }

  /** Sets the sidebar order and group of every project, in the order of `placements`. */
  async arrangeProjects(placements: ProjectPlacement[]) {
    const byId = this.projectsById;
    this.projects = placements.flatMap(({ id, groupId }, sortOrder) => {
      const p = byId.get(id);
      return p ? [{ ...p, groupId: groupId ?? null, sortOrder }] : [];
    });
    const ok = await this.run(() => commands.arrangeProjects(placements));
    if (ok === undefined) await this.load();
  }

  async deleteProject(id: number) {
    const ok = await this.run(() => commands.deleteProject(id));
    if (ok === undefined) return false;
    this.projects = this.projects.filter((p) => p.id !== id);
    this.tickets = this.tickets.filter((t) => t.projectId !== id);
    return true;
  }

  // ---- project groups -------------------------------------------------------

  async createGroup(name: string) {
    const created = await this.run(() => commands.createGroup(name));
    if (created) this.groups = [...this.groups, created];
    return created;
  }

  async updateGroup(id: number, patch: GroupPatch) {
    const updated = await this.run(() => commands.updateGroup(id, patch));
    if (updated) this.groups = this.groups.map((g) => (g.id === id ? updated : g));
    return updated;
  }

  toggleGroupCollapsed(group: ProjectGroup) {
    return this.updateGroup(group.id, { collapsed: !group.collapsed });
  }

  /** Shows or hides all projects of the group. */
  async setGroupHidden(group: ProjectGroup, hidden: boolean) {
    const changed = await this.run(() => commands.setGroupHidden(group.id, hidden));
    if (!changed) return;
    const byId = new Map(changed.map((p) => [p.id, p]));
    this.projects = this.projects.map((p) => byId.get(p.id) ?? p);
  }

  async reorderGroups(ordered: ProjectGroup[]) {
    this.groups = ordered;
    const ok = await this.run(() => commands.reorderGroups(ordered.map((g) => g.id)));
    if (ok === undefined) await this.load();
  }

  /** Deletes the group; its projects become ungrouped. */
  async deleteGroup(id: number) {
    const ok = await this.run(() => commands.deleteGroup(id));
    if (ok === undefined) return false;
    this.groups = this.groups.filter((g) => g.id !== id);
    this.projects = this.projects.map((p) => (p.groupId === id ? { ...p, groupId: null } : p));
    return true;
  }

  // ---- tickets --------------------------------------------------------------

  async createTicket(ticket: NewTicket) {
    const created = await this.run(() => commands.createTicket(ticket));
    if (created) this.tickets = [...this.tickets, created];
    return created;
  }

  async updateTicket(id: number, patch: TicketPatch) {
    const updated = await this.run(() => commands.updateTicket(id, patch));
    if (updated) this.replaceTicket(updated);
    return updated;
  }

  /** Moves a ticket into `to`, directly above `beforeId` (or to the bottom). */
  async moveTicket(id: number, to: TicketStatus, beforeId: number | null = null) {
    const moved = await this.run(() => commands.moveTicket(id, to, beforeId));
    if (moved) this.replaceTicket(moved);
    else await this.load(); // re-sync the board after a refused drop
    return moved;
  }

  async loadTranscript(ticketId: number) {
    const list = await this.run(() => commands.listTicketEvents(ticketId));
    if (list) this.transcripts[ticketId] = list;
  }

  async replyToTicket(id: number, text: string) {
    const updated = await this.run(() => commands.replyToTicket(id, text));
    if (updated) this.replaceTicket(updated);
    return updated;
  }

  async sendBackToReady(id: number, comment: string) {
    const updated = await this.run(() => commands.sendBackToReady(id, comment));
    if (updated) this.replaceTicket(updated);
    return updated;
  }

  /** Saves a pasted image; returns its absolute path, or undefined on failure. */
  async saveAttachment(dataBase64: string, extension: string) {
    return this.run(() => commands.saveAttachment(dataBase64, extension));
  }

  async cancelTicket(id: number) {
    const updated = await this.run(() => commands.cancelTicket(id));
    if (updated) this.replaceTicket(updated);
    return updated;
  }

  async deleteTicket(id: number) {
    const ok = await this.run(() => commands.deleteTicket(id));
    if (ok === undefined) return false;
    this.tickets = this.tickets.filter((t) => t.id !== id);
    delete this.transcripts[id];
    return true;
  }

  private replaceTicket(ticket: Ticket) {
    this.tickets = this.tickets.map((t) => (t.id === ticket.id ? ticket : t));
  }

  /** Runs a backend call, reporting failures in `error`. Returns undefined on failure. */
  private async run<T>(fn: () => Promise<T>): Promise<T | undefined> {
    try {
      this.error = null;
      // Commands without a result resolve to null, so map that to `true` for callers.
      return ((await fn()) ?? true) as T;
    } catch (e) {
      this.error = errorMessage(e);
      return undefined;
    }
  }
}

export const app = new AppState();

// Debugging aid: inspect or drive the state from the dev tools console.
if (import.meta.env.DEV) Object.assign(globalThis, { hivemind: app });
