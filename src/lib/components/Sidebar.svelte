<script lang="ts">
  import { dndzone, type DndEvent } from "svelte-dnd-action";
  import { flip } from "svelte/animate";
  import { app, type Project, type ProjectGroup } from "$lib/app.svelte";
  import { projectColor } from "$lib/theme";
  import Icon from "./Icon.svelte";
  import PathText from "./PathText.svelte";

  let { width }: { width: number } = $props();

  const FLIP_MS = 150;

  /** `dropped` holds a project dragged onto the header of a collapsed group. */
  type GroupItem = { id: number; group: ProjectGroup; projects: Project[]; dropped: Project[] };

  // Local copies that svelte-dnd-action rearranges while dragging.
  let ungrouped = $state<Project[]>([]);
  let groups = $state<GroupItem[]>([]);
  let dragging = $state<"project" | "group" | null>(null);
  $effect.pre(() => {
    const projects = app.projects;
    const known = new Set(app.groups.map((g) => g.id));
    const next = app.groups.map((group) => ({
      id: group.id,
      group,
      projects: projects.filter((p) => p.groupId === group.id),
      dropped: [],
    }));
    const loose = projects.filter((p) => p.groupId == null || !known.has(p.groupId));
    if (dragging) return;
    groups = next;
    ungrouped = loose;
  });

  const ungroupedFirst = $derived(app.settings.ungroupedProjects !== "bottom");

  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  function setZone(groupId: number | null, items: Project[]) {
    if (groupId === null) ungrouped = items;
    else groups = groups.map((g) => (g.id === groupId ? { ...g, projects: items } : g));
  }

  function considerCollapsed(groupId: number, e: CustomEvent<DndEvent<Project>>) {
    dragging = "project";
    groups = groups.map((g) => (g.id === groupId ? { ...g, dropped: e.detail.items } : g));
  }

  /** A collapsed group can't show where the project goes, so it is added at the end. */
  function finalizeCollapsed(groupId: number, e: CustomEvent<DndEvent<Project>>) {
    const added = e.detail.items;
    groups = groups.map((g) =>
      g.id === groupId
        ? {
            ...g,
            projects: [...g.projects.filter((p) => !added.some((a) => a.id === p.id)), ...added],
            dropped: [],
          }
        : g,
    );
    clearTimeout(saveTimer);
    saveTimer = setTimeout(saveProjects);
  }

  function considerProjects(groupId: number | null, e: CustomEvent<DndEvent<Project>>) {
    dragging = "project";
    setZone(groupId, e.detail.items);
  }

  function finalizeProjects(groupId: number | null, e: CustomEvent<DndEvent<Project>>) {
    setZone(groupId, e.detail.items);
    // Moving between groups finalizes both zones; save once both are updated.
    clearTimeout(saveTimer);
    saveTimer = setTimeout(saveProjects);
  }

  async function saveProjects() {
    const placements = [
      ...ungrouped.map((p) => ({ id: p.id, groupId: null })),
      ...groups.flatMap((g) => g.projects.map((p) => ({ id: p.id, groupId: g.id }))),
    ];
    await app.arrangeProjects(placements);
    dragging = null;
  }

  function considerGroups(e: CustomEvent<DndEvent<GroupItem>>) {
    dragging = "group";
    groups = e.detail.items;
  }

  async function finalizeGroups(e: CustomEvent<DndEvent<GroupItem>>) {
    groups = e.detail.items;
    await app.reorderGroups(groups.map((g) => g.group));
    dragging = null;
  }
</script>

<aside class="flex shrink-0 flex-col bg-surface" style:width="{width}px">
  <header class="flex h-12 items-center justify-between px-4">
    <h2 class="text-xs font-semibold tracking-wider text-muted uppercase">Projects</h2>
    <div class="flex gap-1">
      <button
        class="rounded-md p-1.5 text-muted hover:bg-raised hover:text-fg"
        title="New group"
        onclick={() => (app.groupDialog = { mode: "create" })}
      >
        <Icon name="folder-plus" />
      </button>
      <button
        class="rounded-md p-1.5 text-muted hover:bg-raised hover:text-fg"
        title="New project"
        onclick={() => (app.projectDialog = { mode: "create" })}
      >
        <Icon name="plus" />
      </button>
    </div>
  </header>

  {#if app.projects.length === 0 && groups.length === 0}
    <div class="px-4 py-6 text-center text-sm text-muted">
      No projects yet.
      <button
        class="mt-3 block w-full rounded-md bg-accent px-3 py-1.5 text-bg"
        onclick={() => (app.projectDialog = { mode: "create" })}
      >
        Add a project
      </button>
    </div>
  {/if}

  <div class="flex flex-1 flex-col gap-3 overflow-y-auto px-3 pb-3">
    <div class="relative" class:order-last={!ungroupedFirst && groups.length > 0}>
      {#if groups.length > 0 && ungrouped.length === 0 && dragging === "project"}
        <p
          class="pointer-events-none absolute inset-0 flex items-center justify-center rounded-lg border border-dashed border-line text-xs text-muted"
        >
          Drop here to take it out of its group
        </p>
      {/if}
      <ul
        class="space-y-2"
        class:min-h-12={groups.length > 0 && dragging === "project"}
        class:min-h-24={groups.length === 0}
        use:dndzone={{ items: ungrouped, type: "project", flipDurationMs: FLIP_MS, dropTargetStyle: {} }}
        onconsider={(e) => considerProjects(null, e)}
        onfinalize={(e) => finalizeProjects(null, e)}
      >
        {#each ungrouped as project (project.id)}
          <li
            animate:flip={{ duration: FLIP_MS }}
            class="group relative rounded-lg border border-line bg-raised p-3 pl-4"
            style:--project={projectColor(project.colorIndex)}
          >
            {@render projectCard(project)}
          </li>
        {/each}
      </ul>
    </div>

    {#if groups.length > 0}
      <ul
        class="space-y-3"
        use:dndzone={{ items: groups, type: "group", flipDurationMs: FLIP_MS, dropTargetStyle: {} }}
        onconsider={considerGroups}
        onfinalize={finalizeGroups}
      >
        {#each groups as item (item.id)}
          {@const group = item.group}
          {@const allHidden = item.projects.length > 0 && item.projects.every((p) => p.hidden)}
          <li animate:flip={{ duration: FLIP_MS }} class="rounded-lg bg-surface">
            <div class="group/header relative flex items-center gap-1">
              {#if group.collapsed}
                <!-- Drop target over the header, so projects can be dropped into a collapsed group. -->
                <ul
                  class="absolute inset-0 rounded-md"
                  class:pointer-events-none={dragging !== "project"}
                  class:outline-2={item.dropped.length > 0}
                  class:outline-dashed={item.dropped.length > 0}
                  class:outline-accent={item.dropped.length > 0}
                  use:dndzone={{
                    items: item.dropped,
                    type: "project",
                    flipDurationMs: FLIP_MS,
                    dropTargetStyle: {},
                  }}
                  onconsider={(e) => considerCollapsed(group.id, e)}
                  onfinalize={(e) => finalizeCollapsed(group.id, e)}
                >
                  {#each item.dropped as project (project.id)}
                    <li class="invisible absolute inset-0">{project.name}</li>
                  {/each}
                </ul>
              {/if}
              <button
                class="flex min-w-0 flex-1 items-center gap-1.5 rounded-md py-1 text-left hover:text-fg"
                class:opacity-50={allHidden}
                title={group.collapsed ? "Expand" : "Collapse"}
                aria-expanded={!group.collapsed}
                onclick={() => app.toggleGroupCollapsed(group)}
              >
                <span class="shrink-0 text-muted transition-transform" class:rotate-90={!group.collapsed}>
                  <Icon name="chevron-right" size={14} />
                </span>
                <span class="shrink-0 text-muted"><Icon name="folder" size={16} /></span>
                <span class="truncate text-lg font-bold">{group.name}</span>
                <span class="shrink-0 rounded-full bg-raised px-2 text-xs text-muted"
                  >{item.projects.length}</span
                >
              </button>
              <div
                class="flex gap-1 opacity-0 transition-opacity group-hover/header:opacity-100 focus-within:opacity-100"
              >
                <button
                  class="rounded p-1 text-muted hover:bg-raised hover:text-fg"
                  title="New project in this group"
                  onclick={() => (app.projectDialog = { mode: "create", groupId: group.id })}
                >
                  <Icon name="plus" size={14} />
                </button>
                <button
                  class="rounded p-1 text-muted hover:bg-raised hover:text-fg"
                  title="Edit group"
                  onclick={() => (app.groupDialog = { mode: "edit", group })}
                >
                  <Icon name="pencil" size={14} />
                </button>
                <button
                  class="rounded p-1 text-muted hover:bg-raised hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent"
                  title={allHidden ? "Show tickets of all projects" : "Hide tickets of all projects"}
                  disabled={item.projects.length === 0}
                  onclick={() => app.setGroupHidden(group, !allHidden)}
                >
                  <Icon name={allHidden ? "eye-off" : "eye"} size={14} />
                </button>
              </div>
            </div>
            {#if !group.collapsed}
              <div class="relative mt-1 ml-2 border-l border-line pl-2">
                {#if item.projects.length === 0}
                  <p
                    class="pointer-events-none absolute inset-0 ml-2 flex items-center justify-center rounded-lg border border-dashed border-line text-xs text-muted"
                  >
                    Drag projects here
                  </p>
                {/if}
                <ul
                  class="min-h-12 space-y-2"
                  use:dndzone={{
                    items: item.projects,
                    type: "project",
                    flipDurationMs: FLIP_MS,
                    dropTargetStyle: {},
                  }}
                  onconsider={(e) => considerProjects(group.id, e)}
                  onfinalize={(e) => finalizeProjects(group.id, e)}
                >
                  {#each item.projects as project (project.id)}
                    <li
                      animate:flip={{ duration: FLIP_MS }}
                      class="group relative rounded-lg border border-line bg-raised p-3 pl-4"
                      style:--project={projectColor(project.colorIndex)}
                    >
                      {@render projectCard(project)}
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </div>
</aside>

{#snippet projectCard(project: Project)}
  {@const active = app.tickets.some((t) => t.projectId === project.id && t.status === "in_progress")}
  <span class="absolute inset-y-2 left-0 w-1 rounded-r bg-(--project)"></span>
  <div class="flex items-start gap-2" class:opacity-50={project.hidden}>
    <div class="min-w-0 flex-1">
      <div class="truncate text-base font-semibold">{project.name}</div>
      <button
        class="mt-0.5 flex max-w-full cursor-pointer items-center gap-1.5 text-left text-muted hover:text-fg hover:underline [&>svg]:shrink-0"
        title="Open {project.path}"
        onclick={() => app.openFolder(project.path)}
      >
        <Icon name="folder" size={12} />
        <PathText path={project.path} class="font-mono text-xs" />
      </button>
      {#if app.branches[project.id]}
        {#if app.hasGithubDesktop}
          <button
            class="mt-0.5 flex max-w-full cursor-pointer items-center gap-1.5 text-left text-muted hover:text-fg hover:underline [&>svg]:shrink-0"
            title="{app.branches[project.id]} – open in GitHub Desktop"
            onclick={() => app.openInGithubDesktop(project.path)}
          >
            <Icon name="git-branch" size={12} />
            <span class="min-w-0 truncate font-mono text-xs">{app.branches[project.id]}</span>
          </button>
        {:else}
          <div
            class="mt-0.5 flex items-center gap-1.5 text-muted [&>svg]:shrink-0"
            title={app.branches[project.id]}
          >
            <Icon name="git-branch" size={12} />
            <span class="min-w-0 truncate font-mono text-xs">{app.branches[project.id]}</span>
          </div>
        {/if}
      {/if}
    </div>
    <span
      class="mt-1.5 size-2 shrink-0 rounded-full"
      class:bg-(--project)={active}
      class:animate-pulse={active}
      class:bg-line={!active}
      title={active ? "Agent working" : "Idle"}
    ></span>
  </div>
  <div class="mt-2 flex items-center gap-1">
    <button
      class="flex items-center gap-1 rounded border border-line px-1.5 py-0.5 text-xs text-muted hover:bg-surface hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-muted"
      title={project.hidden ? "Show the project to add tickets" : "New ticket"}
      disabled={project.hidden}
      onclick={() => (app.ticketDialog = { mode: "create", projectId: project.id })}
    >
      <Icon name="plus" size={12} />
      Ticket
    </button>
    <div
      class="ml-auto flex gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100"
    >
      <button
        class="rounded p-1 text-muted hover:bg-surface hover:text-fg"
        title="Edit project"
        onclick={() => (app.projectDialog = { mode: "edit", project })}
      >
        <Icon name="pencil" size={14} />
      </button>
      <button
        class="rounded p-1 text-muted hover:bg-surface hover:text-fg"
        title={project.hidden ? "Show tickets" : "Hide tickets"}
        onclick={() => app.toggleProjectHidden(project)}
      >
        <Icon name={project.hidden ? "eye-off" : "eye"} size={14} />
      </button>
    </div>
  </div>
{/snippet}
