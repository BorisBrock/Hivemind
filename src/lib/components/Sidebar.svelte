<script lang="ts">
  import { app } from "$lib/app.svelte";
  import { projectColor } from "$lib/theme";
  import Icon from "./Icon.svelte";
</script>

<aside class="flex w-72 shrink-0 flex-col border-r border-line bg-surface">
  <header class="flex h-12 items-center justify-between px-4">
    <h2 class="text-xs font-semibold tracking-wider text-muted uppercase">Projects</h2>
    <button class="rounded-md p-1.5 text-muted hover:bg-raised hover:text-fg" title="New project" disabled>
      <Icon name="plus" />
    </button>
  </header>

  <ul class="flex-1 space-y-2 overflow-y-auto px-3 pb-3">
    {#each app.projects as project (project.id)}
      {@const active = false}
      <li
        class="group relative rounded-lg border border-line bg-raised p-3 pl-4 transition-opacity"
        class:opacity-50={project.hidden}
        style:--project={projectColor(project.colorIndex)}
      >
        <span class="absolute inset-y-2 left-0 w-1 rounded-r bg-(--project)"></span>
        <div class="flex items-start gap-2">
          <div class="min-w-0 flex-1">
            <div class="truncate text-base font-semibold">{project.name}</div>
            <div class="truncate font-mono text-xs text-muted" title={project.path}>{project.path}</div>
          </div>
          <span
            class="mt-1.5 size-2 shrink-0 rounded-full"
            class:bg-(--project)={active}
            class:animate-pulse={active}
            class:bg-line={!active}
            title={active ? "Agent working" : "Idle"}
          ></span>
        </div>
        <div class="mt-2 flex gap-1 opacity-0 transition-opacity group-hover:opacity-100">
          <button class="rounded p-1 text-muted hover:bg-surface hover:text-fg" title="Edit project" disabled>
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
      </li>
    {:else}
      <li class="px-1 py-6 text-center text-sm text-muted">
        No projects yet.
        {#if import.meta.env.DEV}
          <button
            class="mt-3 block w-full rounded-md bg-accent px-3 py-1.5 text-bg"
            onclick={() => app.seedDemoData()}
          >
            Add demo data
          </button>
        {/if}
      </li>
    {/each}
  </ul>
</aside>
