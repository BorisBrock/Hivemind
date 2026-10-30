<script lang="ts">
  import { dndzone, type DndEvent } from "svelte-dnd-action";
  import { flip } from "svelte/animate";
  import { app } from "$lib/app.svelte";
  import type { Project } from "$lib/bindings";
  import { projectColor } from "$lib/theme";
  import Icon from "./Icon.svelte";
  import PathText from "./PathText.svelte";

  const FLIP_MS = 150;

  // Local copy that svelte-dnd-action rearranges while dragging.
  let items = $state<Project[]>([]);
  let dragging = false;
  $effect.pre(() => {
    const projects = app.projects;
    if (!dragging) items = projects;
  });

  function consider(e: CustomEvent<DndEvent<Project>>) {
    dragging = true;
    items = e.detail.items;
  }

  async function finalize(e: CustomEvent<DndEvent<Project>>) {
    items = e.detail.items;
    dragging = false;
    await app.reorderProjects(items);
  }
</script>

<aside class="flex w-72 shrink-0 flex-col border-r border-line bg-surface">
  <header class="flex h-12 items-center justify-between px-4">
    <h2 class="text-xs font-semibold tracking-wider text-muted uppercase">Projects</h2>
    <button
      class="rounded-md p-1.5 text-muted hover:bg-raised hover:text-fg"
      title="New project"
      onclick={() => (app.projectDialog = { mode: "create" })}
    >
      <Icon name="plus" />
    </button>
  </header>

  {#if items.length === 0}
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

  <ul
    class="flex-1 space-y-2 overflow-y-auto px-3 pb-3"
    use:dndzone={{ items, type: "project", flipDurationMs: FLIP_MS, dropTargetStyle: {} }}
    onconsider={consider}
    onfinalize={finalize}
  >
    {#each items as project (project.id)}
      {@const active = app.tickets.some((t) => t.projectId === project.id && t.status === "in_progress")}
      <li
        animate:flip={{ duration: FLIP_MS }}
        class="group relative rounded-lg border border-line bg-raised p-3 pl-4"
        style:--project={projectColor(project.colorIndex)}
      >
        <span class="absolute inset-y-2 left-0 w-1 rounded-r bg-(--project)"></span>
        <div class="flex items-start gap-2" class:opacity-50={project.hidden}>
          <div class="min-w-0 flex-1">
            <div class="truncate text-base font-semibold">{project.name}</div>
            <div class="mt-0.5 flex items-center gap-1.5 text-muted [&>svg]:shrink-0">
              <Icon name="folder" size={12} />
              <PathText path={project.path} class="font-mono text-xs" />
            </div>
            {#if app.branches[project.id]}
              <div
                class="mt-0.5 flex items-center gap-1.5 text-muted [&>svg]:shrink-0"
                title={app.branches[project.id]}
              >
                <Icon name="git-branch" size={12} />
                <span class="min-w-0 truncate font-mono text-xs">{app.branches[project.id]}</span>
              </div>
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
      </li>
    {/each}
  </ul>
</aside>
