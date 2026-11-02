<script lang="ts">
  import { tick } from "svelte";
  import type { TicketEvent } from "$lib/app.svelte";
  import CopyButton from "./CopyButton.svelte";
  import Markdown from "./Markdown.svelte";

  let {
    events,
    working = false,
    onoption,
  }: {
    events: TicketEvent[];
    /** The agent is running; shows a live indicator at the end. */
    working?: boolean;
    /** Clicking an answer option of the agent's open question. */
    onoption?: (option: string) => void;
  } = $props();

  type Data = TicketEvent["data"];
  type ToolResult = Extract<Data, { kind: "tool_result" }>;
  type Item = { event: TicketEvent; result?: ToolResult };

  // Attach each tool result to its tool call, so a call and its output show as one item.
  const items = $derived.by(() => {
    const out: Item[] = [];
    const calls = new Map<string, Item>();
    for (const event of events) {
      const d = event.data;
      if (d.kind === "tool_result" && calls.has(d.toolUseId)) {
        calls.get(d.toolUseId)!.result = d;
        continue;
      }
      const item: Item = { event };
      if (d.kind === "tool_use") calls.set(d.id, item);
      out.push(item);
    }
    return out;
  });
  // Only the newest question is still open for answers.
  const lastQuestionId = $derived(events.findLast((e) => e.data.kind === "question")?.id);

  let scroller: HTMLDivElement;
  let pinned = true; // follow new output while the user is at the bottom
  $effect(() => {
    void items.length;
    if (pinned) tick().then(() => scroller?.scrollTo({ top: scroller.scrollHeight }));
  });

  function onscroll() {
    pinned = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 40;
  }

  const formatTokens = (n: number) => (n >= 1000 ? `${(n / 1000).toFixed(1)}k` : `${n}`);
</script>

{#snippet code(text: string, classes: string)}
  <div class="code-block">
    <pre class={classes}>{text}</pre>
    <CopyButton {text} />
  </div>
{/snippet}

<div bind:this={scroller} {onscroll} class="h-full space-y-3 overflow-y-auto pr-1">
  {#each items as { event, result } (event.id)}
    {@const d = event.data}
    {#if d.kind === "user_prompt"}
      <div class="rounded-lg border border-(--project)/40 bg-(--project)/8 p-3">
        <div class="mb-1 text-[11px] font-semibold tracking-wider text-project uppercase">You</div>
        <Markdown source={d.text} />
      </div>
    {:else if d.kind === "assistant_text"}
      <div class="px-1"><Markdown source={d.text} /></div>
    {:else if d.kind === "thinking"}
      <details class="px-1 text-sm text-muted">
        <summary class="cursor-pointer select-none">Thinking…</summary>
        <div class="mt-1 border-l-2 border-line pl-3 whitespace-pre-wrap">{d.text}</div>
      </details>
    {:else if d.kind === "tool_use"}
      <details class="group rounded-md border border-line bg-raised text-sm">
        <summary class="flex cursor-pointer items-center gap-2 px-3 py-1.5 select-none">
          <span class="rounded bg-surface px-1.5 py-0.5 font-mono text-[11px] text-accent">{d.name}</span>
          <span class="min-w-0 flex-1 truncate font-mono text-xs text-muted">{d.summary}</span>
          {#if result?.isError}
            <span class="text-xs text-danger">failed</span>
          {:else if !result}
            <span class="text-xs text-muted">…</span>
          {/if}
        </summary>
        <div class="space-y-2 border-t border-line p-3">
          {@render code(d.input, "overflow-x-auto font-mono text-xs whitespace-pre-wrap text-muted")}
          {#if result}
            {@render code(
              result.content || "(no output)",
              `max-h-64 overflow-auto rounded bg-surface p-2 font-mono text-xs whitespace-pre-wrap ${result.isError ? "text-danger" : ""}`,
            )}
          {/if}
        </div>
      </details>
    {:else if d.kind === "tool_result"}
      {@render code(d.content, "rounded bg-raised p-2 font-mono text-xs whitespace-pre-wrap")}
    {:else if d.kind === "question"}
      <div class="rounded-lg border-2 border-accent/60 bg-accent/10 p-3">
        <div class="mb-1 text-[11px] font-semibold tracking-wider text-accent uppercase">The agent asks</div>
        <Markdown source={d.question} />
        {#if d.options.length}
          <div class="mt-2 flex flex-wrap gap-2">
            {#each d.options as option (option)}
              <button
                class="rounded-full border border-accent/60 px-3 py-1 text-sm hover:bg-accent hover:text-bg disabled:opacity-50 disabled:hover:bg-transparent disabled:hover:text-fg"
                disabled={!onoption || event.id !== lastQuestionId}
                onclick={() => onoption?.(option)}
              >
                {option}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {:else if d.kind === "turn_result"}
      <div class="flex items-center gap-3 text-[11px] text-muted" class:text-danger={d.isError}>
        <span class="h-px flex-1 bg-line"></span>
        {d.isError ? "Turn failed" : "Turn finished"} · {(d.durationMs / 1000).toFixed(1)} s ·
        {formatTokens(d.inputTokens)} in / {formatTokens(d.outputTokens)} out
        {#if d.costUsd}· ${d.costUsd.toFixed(2)}{/if}
        <span class="h-px flex-1 bg-line"></span>
      </div>
    {:else if d.kind === "error"}
      <div class="rounded-lg border border-danger/50 bg-danger/10 p-3 text-sm">
        <div class="mb-1 text-[11px] font-semibold tracking-wider text-danger uppercase">Error</div>
        {@render code(d.message, "font-mono text-xs whitespace-pre-wrap")}
      </div>
    {:else if d.kind === "system"}
      <div class="text-center text-xs text-muted">{d.message}</div>
    {/if}
  {/each}

  {#if working}
    <div class="flex items-center gap-2 px-1 text-sm text-muted">
      <span class="size-2 animate-pulse rounded-full bg-(--project)"></span> Agent is working…
    </div>
  {/if}
</div>
