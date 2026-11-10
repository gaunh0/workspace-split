<script lang="ts">
  import type { Space } from "../types";

  let {
    spaces,
    activeSpaceId,
    onSelect,
    onCreate,
    onDelete,
  }: {
    spaces: Space[];
    activeSpaceId: number | null;
    onSelect: (id: number) => void;
    onCreate: (name: string) => Promise<boolean>;
    onDelete: (id: number) => Promise<void>;
  } = $props();

  let name = $state("");
  let submitting = $state(false);

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    const value = name.trim();
    if (!value || submitting) return;

    submitting = true;
    try {
      const created = await onCreate(value);
      if (created) name = "";
    } finally {
      submitting = false;
    }
  }
</script>

<aside class="flex h-full w-72 shrink-0 flex-col border-r border-slate-800/80 bg-slate-900/60">
  <div class="border-b border-slate-800/80 px-5 py-5">
    <div class="flex items-center gap-2.5">
      <span class="grid size-8 place-items-center rounded-lg bg-cyan-400/15 text-cyan-300">
        <svg class="size-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M4 7.5 12 3l8 4.5v9L12 21l-8-4.5v-9Z" />
          <path d="m4 7.5 8 4.5 8-4.5M12 12v9" />
        </svg>
      </span>
      <div>
        <h1 class="text-sm font-semibold tracking-tight text-slate-100">SpaceBox</h1>
        <p class="text-[11px] text-slate-500">Local-first workspace</p>
      </div>
    </div>
  </div>

  <div class="flex min-h-0 flex-1 flex-col">
    <div class="flex items-center justify-between px-5 pt-5 pb-2">
      <span class="text-[11px] font-semibold uppercase tracking-widest text-slate-500">Spaces</span>
      <span class="rounded-full bg-slate-800 px-2 py-0.5 text-[10px] font-medium text-slate-400">
        {spaces.length}
      </span>
    </div>

    <nav class="min-h-0 flex-1 space-y-1 overflow-y-auto px-3 pb-4">
      {#each spaces as space (space.id)}
        <div
          class="group flex items-center rounded-lg transition-colors {space.id === activeSpaceId
            ? 'bg-cyan-400/10 text-cyan-100'
            : 'text-slate-300 hover:bg-slate-800/60'}"
        >
          <button
            type="button"
            class="flex min-w-0 flex-1 items-center gap-2.5 px-3 py-2.5 text-left text-sm"
            aria-current={space.id === activeSpaceId ? "page" : undefined}
            onclick={() => onSelect(space.id)}
          >
            <span
              class="size-1.5 shrink-0 rounded-full {space.id === activeSpaceId
                ? 'bg-cyan-400'
                : 'bg-slate-600'}"
            ></span>
            <span class="truncate">{space.name}</span>
          </button>
          <button
            type="button"
            class="mr-2 hidden rounded-md p-1 text-slate-500 hover:bg-rose-500/10 hover:text-rose-300 group-hover:block"
            title="Delete space"
            aria-label={`Delete ${space.name}`}
            onclick={() => onDelete(space.id)}
          >
            <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3" />
            </svg>
          </button>
        </div>
      {:else}
        <p class="px-3 py-4 text-xs leading-relaxed text-slate-500">
          No spaces yet. Create one below to start grouping files.
        </p>
      {/each}
    </nav>

    <form class="border-t border-slate-800/80 p-3" onsubmit={submit}>
      <label class="sr-only" for="new-space">New space name</label>
      <div class="flex gap-2">
        <input
          id="new-space"
          bind:value={name}
          maxlength="80"
          placeholder="New space…"
          class="min-w-0 flex-1 rounded-lg border border-slate-700/80 bg-slate-950/70 px-3 py-2 text-xs text-slate-200 placeholder:text-slate-600 focus:border-cyan-500/60 focus:outline-none focus:ring-2 focus:ring-cyan-500/20"
        />
        <button
          type="submit"
          disabled={!name.trim() || submitting}
          class="grid size-9 shrink-0 place-items-center rounded-lg bg-cyan-500 text-slate-950 transition hover:bg-cyan-400 disabled:cursor-not-allowed disabled:opacity-40"
          title="Create space"
          aria-label="Create space"
        >
          <svg class="size-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <path d="M12 5v14M5 12h14" />
          </svg>
        </button>
      </div>
    </form>
  </div>
</aside>
