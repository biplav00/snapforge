<script lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { onMount } from "svelte";
import { showToast } from "../toast.ts";

interface HistoryEntry {
  path: string;
  timestamp: string;
  thumbnail_data: string;
  kind: "image" | "video";
  exists: boolean;
}

type SortOrder = "newest" | "oldest" | "name";
type FilterKind = "all" | "image" | "video";

let entries = $state<HistoryEntry[]>([]);
let loading = $state(true);
let previewEntry = $state<HistoryEntry | null>(null);
let search = $state("");
let sortOrder = $state<SortOrder>("newest");
let filterKind = $state<FilterKind>("all");

async function loadHistory() {
  loading = true;
  try {
    const json = await invoke<string>("get_history");
    entries = JSON.parse(json);
  } catch (e) {
    console.error("Failed to load history:", e);
    entries = [];
  }
  loading = false;
}

async function copyToClipboard(entry: HistoryEntry) {
  if (entry.kind === "video") {
    showToast("Videos can't be copied to clipboard — use 'Show in folder' instead");
    return;
  }
  try {
    await invoke("copy_file_to_clipboard", { path: entry.path });
    showToast("Copied to clipboard");
  } catch (e) {
    console.error("Failed to copy:", e);
    showToast("Copy failed");
  }
}

async function openInFolder(path: string) {
  try {
    await invoke("open_file_in_folder", { path });
  } catch (e) {
    console.error("Failed to open file:", e);
  }
}

async function clearAll() {
  if (!window.confirm("Clear all history? This cannot be undone.")) return;
  try {
    await invoke("clear_history");
    entries = [];
    previewEntry = null;
  } catch (e) {
    console.error("Failed to clear history:", e);
  }
}

async function deleteEntry(entry: HistoryEntry) {
  try {
    await invoke("delete_history_entry", { path: entry.path });
    entries = entries.filter((e) => e.path !== entry.path);
    if (previewEntry?.path === entry.path) {
      previewEntry = null;
    }
  } catch (e) {
    console.error("Failed to delete entry:", e);
  }
}

function formatDate(timestamp: string): string {
  try {
    const d = new Date(timestamp);
    const now = new Date();
    const sameDay = d.toDateString() === now.toDateString();
    if (sameDay) {
      return d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
    }
    return d.toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return timestamp;
  }
}

function fileName(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/");
  return parts[parts.length - 1] || path;
}

const filteredSorted = $derived.by(() => {
  let result = [...entries];

  if (filterKind !== "all") {
    result = result.filter((e) => e.kind === filterKind);
  }

  if (search.trim()) {
    const q = search.trim().toLowerCase();
    result = result.filter((e) => fileName(e.path).toLowerCase().includes(q));
  }

  result.sort((a, b) => {
    if (sortOrder === "name") {
      return fileName(a.path).localeCompare(fileName(b.path));
    }
    const cmp = a.timestamp.localeCompare(b.timestamp);
    return sortOrder === "newest" ? -cmp : cmp;
  });

  return result;
});

function handleGlobalKeydown(e: KeyboardEvent) {
  if (e.key === "Escape" && previewEntry) {
    previewEntry = null;
  }
  // Focus search on / or Cmd+F
  if ((e.metaKey || e.ctrlKey) && e.key === "f") {
    e.preventDefault();
    const input = document.getElementById("history-search") as HTMLInputElement | null;
    input?.focus();
  }
}

onMount(() => {
  loadHistory();
  window.addEventListener("keydown", handleGlobalKeydown);
  return () => window.removeEventListener("keydown", handleGlobalKeydown);
});
</script>

<div class="app">
  <header class="toolbar">
    <h1>History</h1>
    <div class="toolbar-controls">
      <div class="search-wrapper">
        <svg class="search-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="7" cy="7" r="5"></circle>
          <path d="M11 11l4 4"></path>
        </svg>
        <input
          id="history-search"
          type="text"
          placeholder="Search by filename…"
          bind:value={search}
          class="search-input"
        />
      </div>
      <select class="filter-select" bind:value={filterKind}>
        <option value="all">All</option>
        <option value="image">Images</option>
        <option value="video">Videos</option>
      </select>
      <select class="filter-select" bind:value={sortOrder}>
        <option value="newest">Newest</option>
        <option value="oldest">Oldest</option>
        <option value="name">Name</option>
      </select>
    </div>
  </header>

  <main class="content">
    {#if loading}
      <div class="empty-state">
        <div class="spinner"></div>
        <p>Loading history…</p>
      </div>
    {:else if entries.length === 0}
      <div class="empty-state">
        <svg viewBox="0 0 48 48" width="48" height="48" fill="none" stroke="currentColor" stroke-width="1.5" opacity="0.3">
          <rect x="6" y="10" width="36" height="28" rx="3"></rect>
          <circle cx="16" cy="20" r="3"></circle>
          <path d="M6 32l10-10 10 10 6-6 10 10"></path>
        </svg>
        <p class="empty-title">No history yet</p>
        <p class="empty-hint">Screenshots and recordings will appear here after you save them.</p>
      </div>
    {:else if filteredSorted.length === 0}
      <div class="empty-state">
        <p class="empty-title">No matches</p>
        <p class="empty-hint">Try a different search or filter.</p>
      </div>
    {:else}
      <div class="grid-scroll">
        <div class="grid">
          {#each filteredSorted as entry (entry.path)}
            <article class="card" class:missing={!entry.exists} title={entry.path}>
              <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
              <div class="thumb" onclick={() => { previewEntry = entry; }}>
                {#if entry.thumbnail_data}
                  <img src={entry.thumbnail_data} alt={fileName(entry.path)} loading="lazy" />
                {:else if entry.kind === "video"}
                  <div class="placeholder video-placeholder">
                    <svg viewBox="0 0 24 24" width="28" height="28" fill="currentColor">
                      <path d="M8 5v14l11-7z"/>
                    </svg>
                  </div>
                {:else}
                  <div class="placeholder">No preview</div>
                {/if}
                {#if entry.kind === "video"}
                  <div class="badge">VIDEO</div>
                {/if}
                {#if !entry.exists}
                  <div class="badge missing-badge">MISSING</div>
                {/if}
              </div>
              <div class="meta">
                <div class="meta-text">
                  <div class="filename">{fileName(entry.path)}</div>
                  <div class="date">{formatDate(entry.timestamp)}</div>
                </div>
                <div class="actions">
                  {#if entry.kind === "image" && entry.exists}
                    <button class="action" title="Copy" onclick={() => copyToClipboard(entry)}>
                      <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5">
                        <rect x="4" y="4" width="9" height="9" rx="1"></rect>
                        <path d="M3 11V3a1 1 0 0 1 1-1h8"></path>
                      </svg>
                    </button>
                  {/if}
                  {#if entry.exists}
                    <button class="action" title="Show in folder" onclick={() => openInFolder(entry.path)}>
                      <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor">
                        <path d="M0 2.75C0 1.784.784 1 1.75 1H5c.55 0 1.07.26 1.4.7l.9 1.2a.25.25 0 0 0 .2.1h6.75c.966 0 1.75.784 1.75 1.75v8.5A1.75 1.75 0 0 1 14.25 15H1.75A1.75 1.75 0 0 1 0 13.25Z"/>
                      </svg>
                    </button>
                  {/if}
                  <button class="action danger" title="Delete from history" onclick={() => deleteEntry(entry)}>
                    <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.5">
                      <path d="M3 5h10M6 5V3h4v2M5 5l1 9h4l1-9"></path>
                    </svg>
                  </button>
                </div>
              </div>
            </article>
          {/each}
        </div>
      </div>
    {/if}
  </main>

  <footer class="footer">
    <span class="footer-count">
      {filteredSorted.length} of {entries.length}
      {entries.length === 1 ? "item" : "items"}
    </span>
    {#if entries.length > 0}
      <button class="btn-clear" onclick={clearAll}>Clear All</button>
    {/if}
  </footer>

  {#if previewEntry}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="preview-overlay" onclick={() => { previewEntry = null; }}>
      <div class="preview-container" onclick={(e) => e.stopPropagation()}>
        <div class="preview-header">
          <span class="preview-name">{fileName(previewEntry.path)}</span>
          <button class="preview-close" onclick={() => { previewEntry = null; }}>×</button>
        </div>
        <div class="preview-body">
          {#if previewEntry.kind === "image" && previewEntry.thumbnail_data}
            <img src={previewEntry.thumbnail_data} alt={fileName(previewEntry.path)} />
          {:else if previewEntry.kind === "video"}
            <div class="preview-video-placeholder">
              <svg viewBox="0 0 64 64" width="64" height="64" fill="currentColor">
                <path d="M20 14v36l28-18z"/>
              </svg>
              <p>Video preview not available</p>
              <p class="hint">Open in folder to play</p>
            </div>
          {:else}
            <p>No preview available</p>
          {/if}
        </div>
        <div class="preview-footer">
          {#if previewEntry.kind === "image" && previewEntry.exists}
            <button class="preview-btn" onclick={() => previewEntry && copyToClipboard(previewEntry)}>Copy</button>
          {/if}
          {#if previewEntry.exists}
            <button class="preview-btn" onclick={() => previewEntry && openInFolder(previewEntry.path)}>Show in Folder</button>
          {/if}
          <button class="preview-btn danger" onclick={() => previewEntry && deleteEntry(previewEntry)}>Delete</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(html), :global(body) {
    background: var(--bg);
    color: var(--text);
  }

  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    font-family: system-ui, -apple-system, sans-serif;
    background: var(--bg);
    color: var(--text);
  }

  /* Toolbar */
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 14px 20px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  h1 {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
    margin: 0;
  }

  .toolbar-controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .search-wrapper {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    color: var(--text-muted);
    pointer-events: none;
  }

  .search-input {
    padding: 6px 10px 6px 30px;
    width: 220px;
    font-size: 12px;
    font-family: inherit;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text);
    outline: none;
    transition: border-color 0.15s;
  }

  .search-input:focus {
    border-color: var(--accent);
  }

  .filter-select {
    padding: 6px 10px;
    font-size: 12px;
    font-family: inherit;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: 6px;
    color: var(--text);
    cursor: pointer;
    outline: none;
  }

  .filter-select:hover {
    border-color: var(--accent);
  }

  /* Content area */
  .content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  .grid-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 14px;
    padding: 16px 20px 20px;
  }

  /* Custom scrollbar */
  .grid-scroll::-webkit-scrollbar {
    width: 10px;
  }
  .grid-scroll::-webkit-scrollbar-track {
    background: transparent;
  }
  .grid-scroll::-webkit-scrollbar-thumb {
    background: var(--border);
    border-radius: 5px;
  }
  .grid-scroll::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }

  /* Card */
  .card {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    transition: transform 0.15s, border-color 0.15s, box-shadow 0.15s;
    display: flex;
    flex-direction: column;
  }

  .card:hover {
    transform: translateY(-2px);
    border-color: var(--accent);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
  }

  .card.missing {
    opacity: 0.5;
  }

  .thumb {
    position: relative;
    aspect-ratio: 16 / 10;
    background: var(--bg-input);
    cursor: pointer;
    overflow: hidden;
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }

  .placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    color: var(--text-faint);
  }

  .video-placeholder {
    color: var(--accent);
    background: linear-gradient(135deg, var(--bg-input), var(--bg-secondary));
  }

  .badge {
    position: absolute;
    top: 8px;
    left: 8px;
    padding: 2px 6px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.5px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.75);
    color: white;
    backdrop-filter: blur(4px);
  }

  .missing-badge {
    top: auto;
    bottom: 8px;
    background: rgba(204, 68, 68, 0.85);
  }

  /* Meta section */
  .meta {
    padding: 8px 10px 10px;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .meta-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .filename {
    font-size: 11px;
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .date {
    font-size: 10px;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    gap: 2px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .card:hover .actions {
    opacity: 1;
  }

  .action {
    width: 22px;
    height: 22px;
    padding: 0;
    background: transparent;
    border: none;
    color: var(--text-muted);
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.1s, color 0.1s;
  }

  .action:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .action.danger:hover {
    background: var(--danger-bg);
    color: var(--danger);
  }

  /* Empty state */
  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--text-muted);
    padding: 40px;
  }

  .empty-title {
    font-size: 14px;
    font-weight: 500;
    color: var(--text);
    margin: 0;
  }

  .empty-hint {
    font-size: 12px;
    color: var(--text-faint);
    margin: 0;
    text-align: center;
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  /* Footer */
  .footer {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 20px;
    border-top: 1px solid var(--border);
    background: var(--bg-footer);
  }

  .footer-count {
    font-size: 11px;
    color: var(--text-muted);
  }

  .btn-clear {
    background: transparent;
    color: var(--danger);
    border: 1px solid var(--danger-border);
    border-radius: 5px;
    padding: 5px 12px;
    font-size: 11px;
    font-family: inherit;
    cursor: pointer;
    transition: background 0.15s, color 0.15s;
  }

  .btn-clear:hover {
    background: var(--danger);
    color: white;
  }

  /* Preview modal */
  .preview-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.85);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 40px;
    backdrop-filter: blur(4px);
  }

  .preview-container {
    max-width: 90vw;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
  }

  .preview-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
    gap: 12px;
  }

  .preview-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .preview-close {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 22px;
    line-height: 1;
    cursor: pointer;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
  }

  .preview-close:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .preview-body {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #0a0a0a;
    padding: 20px;
  }

  .preview-body img {
    max-width: 100%;
    max-height: 70vh;
    object-fit: contain;
    display: block;
  }

  .preview-video-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--text-muted);
  }

  .preview-video-placeholder .hint {
    font-size: 11px;
    color: var(--text-faint);
    margin: 0;
  }

  .preview-footer {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding: 10px 16px;
    border-top: 1px solid var(--border);
  }

  .preview-btn {
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--bg-input);
    color: var(--text);
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
  }

  .preview-btn:hover {
    background: var(--bg-hover);
    border-color: var(--accent);
  }

  .preview-btn.danger {
    color: var(--danger);
    border-color: var(--danger-border);
  }

  .preview-btn.danger:hover {
    background: var(--danger);
    color: white;
  }
</style>
