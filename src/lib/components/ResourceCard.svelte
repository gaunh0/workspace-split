<script lang="ts">
  import type { Resource } from "../types";

  let {
    resource,
    onRemove,
    onReveal,
    onSaveNote,
  }: {
    resource: Resource;
    onRemove: (resource: Resource) => Promise<void>;
    onReveal: (resource: Resource) => Promise<void>;
    onSaveNote: (resource: Resource, note: string) => Promise<void>;
  } = $props();

  let note = $state("");
  let editing = $state(false);
  let saving = $state(false);

  $effect(() => {
    if (!editing) {
      note = resource.note;
    }
  });

  async function save() {
    if (saving) return;
    saving = true;
    try {
      if (note !== resource.note) {
        await onSaveNote(resource, note);
      }
      editing = false;
    } finally {
      saving = false;
    }
  }

  function cancel() {
    note = resource.note;
    editing = false;
  }
</script>

<article
  class="group rounded-xl border border-slate-800/80 bg-slate-900/40 p-4 transition hover:border-slate-700 hover:bg-slate-900/70"
>
  <div class="flex items-start gap-3">
    <span
      class="mt-0.5 grid size-9 shrink-0 place-items-center rounded-lg {resource.type === 'folder'
        ? 'bg-amber-400/10 text-amber-300'
        : 'bg-sky-400/10 text-sky-300'}"
    >
      {#if resource.type === "folder"}
        <svg class="size-4.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
          <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" />
        </svg>
      {:else}
        <svg class="size-4.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
          <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8l-5-5Z" />
          <path d="M14 3v5h5" />
        </svg>
      {/if}
    </span>

    <div class="min-w-0 flex-1">
      <div class="flex items-start justify-between gap-2">
        <div class="min-w-0">
          <h3 class="truncate text-sm font-medium text-slate-100" title={resource.title}>
            {resource.title}
          </h3>
          <p class="mt-0.5 truncate font-mono text-[11px] text-slate-500" title={resource.path}>
            {resource.path}
          </p>
        </div>

        <div class="flex shrink-0 items-center gap-1 opacity-0 transition group-hover:opacity-100">
          <button
            type="button"
            class="rounded-md p-1.5 text-slate-500 hover:bg-slate-800 hover:text-slate-200"
            title="Show in Explorer"
            aria-label="Show in Explorer"
            onclick={() => onReveal(resource)}
          >
            <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="m5 3 14 9-6 1.5L10 20 5 3Z" />
            </svg>
          </button>
          <button
            type="button"
            class="rounded-md p-1.5 text-slate-500 hover:bg-rose-500/10 hover:text-rose-300"
            title="Remove from space"
            aria-label="Remove from space"
            onclick={() => onRemove(resource)}
          >
            <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M6 6l12 12M18 6 6 18" />
            </svg>
          </button>
        </div>
      </div>

      <div class="mt-2 flex flex-wrap items-center gap-1.5">
        <span
          class="rounded-full px-2 py-0.5 text-[10px] font-medium uppercase tracking-wide {resource.type === 'folder'
            ? 'bg-amber-400/10 text-amber-300/90'
            : 'bg-sky-400/10 text-sky-300/90'}"
        >
          {resource.type}
        </span>
        {#if resource.isMissing}
          <span class="rounded-full bg-rose-500/15 px-2 py-0.5 text-[10px] font-medium text-rose-300">
            missing
          </span>
        {/if}
      </div>

      <div class="mt-3">
        {#if editing}
          <textarea
            bind:value={note}
            rows="3"
            placeholder="Why is this here?"
            class="w-full resize-none rounded-lg border border-slate-700 bg-slate-950/80 px-2.5 py-2 text-xs text-slate-200 placeholder:text-slate-600 focus:border-cyan-500/60 focus:outline-none focus:ring-2 focus:ring-cyan-500/20"
          ></textarea>
          <div class="mt-2 flex justify-end gap-2">
            <button
              type="button"
              class="rounded-md px-2 py-1 text-[11px] text-slate-400 hover:bg-slate-800"
              onclick={cancel}
            >
              Cancel
            </button>
            <button
              type="button"
              disabled={saving}
              class="rounded-md bg-cyan-500/90 px-2.5 py-1 text-[11px] font-medium text-slate-950 hover:bg-cyan-400 disabled:opacity-50"
              onclick={save}
            >
              Save note
            </button>
          </div>
        {:else if resource.note}
          <button
            type="button"
            class="w-full rounded-lg border border-transparent bg-slate-950/40 px-2.5 py-2 text-left text-xs leading-relaxed text-slate-400 hover:border-slate-800 hover:text-slate-300"
            onclick={() => (editing = true)}
          >
            {resource.note}
          </button>
        {:else}
          <button
            type="button"
            class="text-[11px] font-medium text-slate-600 hover:text-cyan-300"
            onclick={() => (editing = true)}
          >
            + Add note
          </button>
        {/if}
      </div>
    </div>
  </div>
</article>
