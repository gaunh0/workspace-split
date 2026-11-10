<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";

  import * as api from "./lib/api";
  import { errorMessage } from "./lib/api";
  import type { MissingReport, Resource, Space } from "./lib/types";
  import ResourceCard from "./lib/components/ResourceCard.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";

  let spaces = $state<Space[]>([]);
  let activeSpaceId = $state<number | null>(null);
  let resources = $state<Resource[]>([]);
  let query = $state("");
  let loading = $state(false);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let report = $state<MissingReport | null>(null);

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  let requestId = 0;

  const activeSpace = $derived(
    spaces.find((space) => space.id === activeSpaceId) ?? null,
  );
  const missingCount = $derived(resources.filter((item) => item.isMissing).length);
  const searching = $derived(query.trim().length > 0);

  onMount(() => {
    void bootstrap();

    const unlisten = listen("resources-changed", () => {
      void refreshResources();
    });

    return () => {
      if (searchTimer) clearTimeout(searchTimer);
      void unlisten.then((stop) => stop());
    };
  });

  async function bootstrap() {
    try {
      spaces = await api.listSpaces();
      if (spaces.length > 0) {
        activeSpaceId = spaces[0].id;
        await refreshResources();
      }
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function refreshResources() {
    const spaceId = activeSpaceId;
    if (spaceId === null) {
      resources = [];
      return;
    }

    const current = ++requestId;
    loading = true;
    try {
      const term = query.trim();
      const data = term
        ? await api.searchResources(term, spaceId)
        : await api.listResources(spaceId);

      if (current === requestId) {
        resources = data;
      }
    } catch (cause) {
      if (current === requestId) {
        error = errorMessage(cause);
      }
    } finally {
      if (current === requestId) {
        loading = false;
      }
    }
  }

  function scheduleSearch() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      void refreshResources();
    }, 180);
  }

  function selectSpace(id: number) {
    if (id === activeSpaceId) return;
    activeSpaceId = id;
    query = "";
    report = null;
    resources = [];
    void refreshResources();
  }

  async function createSpace(name: string): Promise<boolean> {
    try {
      const space = await api.createSpace(name);
      spaces = [...spaces, space].sort((left, right) =>
        left.name.localeCompare(right.name),
      );
      selectSpace(space.id);
      return true;
    } catch (cause) {
      error = errorMessage(cause);
      return false;
    }
  }

  async function deleteSpace(id: number) {
    const space = spaces.find((item) => item.id === id);
    if (!space) return;
    if (!confirm(`Delete "${space.name}"? Files on disk are not touched.`)) return;

    try {
      await api.deleteSpace(id);
      spaces = spaces.filter((item) => item.id !== id);

      if (activeSpaceId === id) {
        activeSpaceId = spaces[0]?.id ?? null;
        query = "";
        report = null;
        resources = [];
        await refreshResources();
      }
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function addPaths(paths: string[]) {
    const spaceId = activeSpaceId;
    if (spaceId === null || paths.length === 0) return;

    busy = true;
    const failures: string[] = [];

    for (const path of paths) {
      try {
        await api.addResource(spaceId, path);
      } catch (cause) {
        failures.push(errorMessage(cause));
      }
    }

    busy = false;
    if (failures.length > 0) {
      error = failures.join("\n");
    }
    await refreshResources();
  }

  async function addFiles() {
    const selection = await open({
      multiple: true,
      directory: false,
      title: "Add files to this space",
    });
    if (!selection) return;

    await addPaths(Array.isArray(selection) ? selection : [selection]);
  }

  async function addFolder() {
    const selection = await open({
      directory: true,
      multiple: false,
      title: "Add a folder to this space",
    });
    if (typeof selection !== "string") return;

    await addPaths([selection]);
  }

  async function checkMissing() {
    if (activeSpaceId === null) return;

    busy = true;
    try {
      report = await api.checkMissingFiles(activeSpaceId);
      await refreshResources();
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = false;
    }
  }

  async function removeResource(resource: Resource) {
    try {
      await api.deleteResource(resource.id);
      resources = resources.filter((item) => item.id !== resource.id);
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function saveNote(resource: Resource, note: string) {
    try {
      const updated = await api.updateResource(resource.id, resource.title, note);
      resources = resources.map((item) =>
        item.id === updated.id ? updated : item,
      );
    } catch (cause) {
      error = errorMessage(cause);
    }
  }

  async function revealResource(resource: Resource) {
    try {
      await api.revealInExplorer(resource.path);
    } catch (cause) {
      error = errorMessage(cause);
    }
  }
</script>

<div class="flex h-full overflow-hidden">
  <Sidebar
    {spaces}
    {activeSpaceId}
    onSelect={selectSpace}
    onCreate={createSpace}
    onDelete={deleteSpace}
  />

  <main class="flex min-w-0 flex-1 flex-col bg-slate-950">
    <header class="border-b border-slate-800/80 bg-slate-950/80 px-6 py-4 backdrop-blur">
      <div class="flex items-center gap-4">
        <div class="min-w-0 flex-1">
          <h2 class="truncate text-base font-semibold tracking-tight text-slate-100">
            {activeSpace?.name ?? "No space selected"}
          </h2>
          <p class="mt-0.5 text-[11px] text-slate-500">
            {#if activeSpace}
              {resources.length} item{resources.length === 1 ? "" : "s"} · paths only, nothing copied
            {:else}
              Create a space to start grouping files and folders
            {/if}
          </p>
        </div>

        <div class="flex shrink-0 items-center gap-2">
          <button
            type="button"
            disabled={!activeSpace || busy}
            class="inline-flex items-center gap-1.5 rounded-lg border border-slate-700/80 bg-slate-900 px-3 py-2 text-xs font-medium text-slate-300 transition hover:border-slate-600 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
            onclick={addFiles}
          >
            <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M12 5v14M5 12h14" />
            </svg>
            Files
          </button>

          <button
            type="button"
            disabled={!activeSpace || busy}
            class="inline-flex items-center gap-1.5 rounded-lg border border-slate-700/80 bg-slate-900 px-3 py-2 text-xs font-medium text-slate-300 transition hover:border-slate-600 hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
            onclick={addFolder}
          >
            <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z" />
              <path d="M12 11v5M9.5 13.5h5" />
            </svg>
            Folder
          </button>

          <button
            type="button"
            disabled={!activeSpace || busy}
            class="inline-flex items-center gap-1.5 rounded-lg bg-cyan-500 px-3 py-2 text-xs font-semibold text-slate-950 transition hover:bg-cyan-400 disabled:cursor-not-allowed disabled:opacity-40"
            onclick={checkMissing}
          >
            <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
              <path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" />
            </svg>
            Check missing
          </button>
        </div>
      </div>

      <div class="mt-4 flex items-center gap-3">
        <div class="relative flex-1">
          <svg
            class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-slate-600"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
          >
            <circle cx="11" cy="11" r="7" />
            <path d="m20 20-3.5-3.5" />
          </svg>
          <input
            bind:value={query}
            oninput={scheduleSearch}
            disabled={!activeSpace}
            placeholder="Search title, note or path (FTS5)…"
            class="w-full rounded-lg border border-slate-800 bg-slate-900/70 py-2.5 pr-3 pl-9 text-sm text-slate-200 placeholder:text-slate-600 focus:border-cyan-500/60 focus:outline-none focus:ring-2 focus:ring-cyan-500/20 disabled:opacity-40"
          />
          {#if searching}
            <button
              type="button"
              class="absolute top-1/2 right-2 -translate-y-1/2 rounded-md px-2 py-1 text-[10px] font-medium text-slate-500 hover:bg-slate-800 hover:text-slate-300"
              onclick={() => {
                query = "";
                void refreshResources();
              }}
            >
              clear
            </button>
          {/if}
        </div>
      </div>
    </header>

    <section class="min-h-0 flex-1 overflow-y-auto px-6 py-5">
      {#if error}
        <div class="mb-4 flex items-start gap-3 rounded-lg border border-rose-500/30 bg-rose-500/10 px-4 py-3">
          <p class="flex-1 whitespace-pre-line text-xs leading-relaxed text-rose-200">{error}</p>
          <button
            type="button"
            class="rounded-md p-1 text-rose-300/70 hover:bg-rose-500/10 hover:text-rose-200"
            aria-label="Dismiss error"
            onclick={() => (error = null)}
          >
            <svg class="size-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M6 6l12 12M18 6 6 18" />
            </svg>
          </button>
        </div>
      {/if}

      {#if report}
        <div class="mb-4 flex items-center gap-3 rounded-lg border border-cyan-500/25 bg-cyan-500/5 px-4 py-2.5">
          <p class="flex-1 text-xs text-cyan-100/80">
            Checked {report.checked} path{report.checked === 1 ? "" : "s"} ·
            <span class="text-rose-300">{report.markedMissing} missing</span> ·
            <span class="text-emerald-300">{report.recovered} back</span>
          </p>
          <button
            type="button"
            class="text-[10px] font-medium text-cyan-300/70 hover:text-cyan-200"
            onclick={() => (report = null)}
          >
            dismiss
          </button>
        </div>
      {/if}

      {#if !activeSpace}
        <div class="grid h-full place-items-center">
          <div class="max-w-sm text-center">
            <div class="mx-auto grid size-14 place-items-center rounded-2xl bg-slate-900 text-slate-600">
              <svg class="size-7" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
                <path d="M4 7.5 12 3l8 4.5v9L12 21l-8-4.5v-9Z" />
                <path d="m4 7.5 8 4.5 8-4.5M12 12v9" />
              </svg>
            </div>
            <h3 class="mt-4 text-sm font-semibold text-slate-200">A workspace, not a copy</h3>
            <p class="mt-1.5 text-xs leading-relaxed text-slate-500">
              Create a space, then link files and folders in place. ClingSpace stores only
              absolute paths and metadata in SQLite.
            </p>
          </div>
        </div>
      {:else if loading}
        <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {#each Array(6) as _}
            <div class="h-36 animate-pulse rounded-xl border border-slate-800/60 bg-slate-900/40"></div>
          {/each}
        </div>
      {:else if resources.length === 0}
        <div class="grid h-full place-items-center">
          <div class="max-w-sm text-center">
            <div class="mx-auto grid size-14 place-items-center rounded-2xl bg-slate-900 text-slate-600">
              <svg class="size-7" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6">
                <path d="M4 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7Z" />
                <path d="M12 11v5M9.5 13.5h5" />
              </svg>
            </div>
            <h3 class="mt-4 text-sm font-semibold text-slate-200">
              {searching ? "No matches" : "This space is empty"}
            </h3>
            <p class="mt-1.5 text-xs leading-relaxed text-slate-500">
              {#if searching}
                Nothing matched “{query.trim()}”. Try a shorter prefix — FTS5 handles prefix
                search.
              {:else}
                Add files or a folder. They stay exactly where they are on disk.
              {/if}
            </p>
          </div>
        </div>
      {:else}
        <div class="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {#each resources as resource (resource.id)}
            <ResourceCard
              {resource}
              onRemove={removeResource}
              onReveal={revealResource}
              onSaveNote={saveNote}
            />
          {/each}
        </div>
      {/if}
    </section>

    <footer
      class="flex items-center justify-between border-t border-slate-800/80 px-6 py-2.5 text-[10px] text-slate-600"
    >
      <span class="inline-flex items-center gap-1.5">
        <span class="size-1.5 rounded-full {missingCount > 0 ? 'bg-amber-400' : 'bg-emerald-500'}"></span>
        {#if missingCount > 0}
          {missingCount} missing path{missingCount === 1 ? "" : "s"} in view
        {:else}
          All visible paths resolve
        {/if}
      </span>
      <span>SQLite FTS5 · notify (ReadDirectoryChangesW) · {activeSpace ? "watching" : "idle"}</span>
    </footer>
  </main>
</div>
