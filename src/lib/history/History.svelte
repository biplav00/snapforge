<script lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { onMount } from "svelte";
import { showToast } from "../toast.ts";

interface HistoryEntry {
  path: string;
  timestamp: string;
  thumbnail_data: string;
}

let entries = $state<HistoryEntry[]>([]);
let loading = $state(true);
let previewEntry = $state<HistoryEntry | null>(null);

async function loadHistory() {
  loading = true;
  try {
    const json = await invoke<string>("get_history");
    entries = JSON.parse(json);
    entries.reverse();
  } catch (e) {
    console.error("Failed to load history:", e);
    entries = [];
  }
  loading = false;
}

async function copyToClipboard(path: string) {
  try {
    await invoke("copy_file_to_clipboard", { path });
    showToast("Copied to clipboard!");
  } catch (e) {
    console.error("Failed to copy:", e);
    showToast("Copy failed");
  }
}

async function openFile(path: string) {
  try {
    await invoke("open_file_in_folder", { path });
  } catch (e) {
    console.error("Failed to open file:", e);
  }
}

async function clearHistory() {
  try {
    await invoke("clear_history");
    entries = [];
  } catch (e) {
    console.error("Failed to clear history:", e);
  }
}

function formatDate(timestamp: string): string {
  try {
    const d = new Date(timestamp);
    return d.toLocaleDateString(undefined, {
      year: "numeric",
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

onMount(() => {
  loadHistory();
});
</script>

<div class="history-container">
  <header>
    <h1>Screenshot History</h1>
  </header>

  {#if loading}
    <div class="empty-state">Loading...</div>
  {:else if entries.length === 0}
    <div class="empty-state">
      <p>No screenshots yet.</p>
      <p class="hint">Screenshots will appear here after you save or copy them.</p>
    </div>
  {:else}
    <div class="grid">
      {#each entries as entry}
        <div class="grid-item" title={entry.path}>
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="thumb-wrapper" onclick={() => copyToClipboard(entry.path)}>
            {#if entry.thumbnail_data}
              <img
                class="thumbnail"
                src={entry.thumbnail_data}
                alt={fileName(entry.path)}
              />
            {:else}
              <div class="thumbnail placeholder">No preview</div>
            {/if}
            <div class="overlay-actions">
              <button
                class="icon-btn"
                title="Preview"
                onclick={(e) => { e.stopPropagation(); previewEntry = entry; }}
              >
                <svg viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.5">
                  <path d="M2 14L14 2M14 2H6M14 2v8"/>
                </svg>
              </button>
              <button
                class="icon-btn"
                title="Show in folder"
                onclick={(e) => { e.stopPropagation(); openFile(entry.path); }}
              >
                <svg viewBox="0 0 16 16" width="14" height="14" fill="currentColor">
                  <path d="M0 2.75C0 1.784.784 1 1.75 1H5c.55 0 1.07.26 1.4.7l.9 1.2a.25.25 0 0 0 .2.1h6.75c.966 0 1.75.784 1.75 1.75v8.5A1.75 1.75 0 0 1 14.25 15H1.75A1.75 1.75 0 0 1 0 13.25Zm1.75-.25a.25.25 0 0 0-.25.25v10.5c0 .138.112.25.25.25h12.5a.25.25 0 0 0 .25-.25v-8.5a.25.25 0 0 0-.25-.25H7.5c-.55 0-1.07-.26-1.4-.7l-.9-1.2a.25.25 0 0 0-.2-.1Z"/>
                </svg>
              </button>
            </div>
          </div>
          <div class="info">
            <span class="filename">{fileName(entry.path)}</span>
            <span class="date">{formatDate(entry.timestamp)}</span>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  {#if previewEntry}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="preview-overlay" onclick={() => { previewEntry = null; }}>
      <div class="preview-container" onclick={(e) => e.stopPropagation()}>
        <img
          class="preview-img"
          src={previewEntry.thumbnail_data}
          alt={fileName(previewEntry.path)}
        />
        <div class="preview-bar">
          <span class="preview-name">{fileName(previewEntry.path)}</span>
          <div class="preview-actions">
            <button class="preview-btn" onclick={() => { if (previewEntry) copyToClipboard(previewEntry.path); }}>Copy</button>
            <button class="preview-btn" onclick={() => { if (previewEntry) openFile(previewEntry.path); }}>Show in Folder</button>
            <button class="preview-btn close" onclick={() => { previewEntry = null; }}>Close</button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  {#if entries.length > 0}
    <footer>
      <span class="count">{entries.length} screenshot{entries.length === 1 ? "" : "s"}</span>
      <button class="btn-clear" onclick={clearHistory}>Clear History</button>
    </footer>
  {/if}
</div>

<style>
  .history-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    font-family: system-ui, -apple-system, sans-serif;
  }

  header {
    padding: 16px 20px 12px;
    border-bottom: 1px solid var(--border);
  }

  h1 {
    font-size: 16px;
    font-weight: 600;
    color: var(--text);
  }

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    font-size: 14px;
    gap: 6px;
  }

  .hint {
    font-size: 12px;
    color: var(--text-faint);
  }

  .grid {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
    align-content: start;
  }

  .grid-item {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    transition: border-color 0.15s, background 0.15s;
    display: flex;
    flex-direction: column;
  }

  .grid-item:hover {
    border-color: var(--accent);
    background: var(--bg-hover);
  }

  .thumb-wrapper {
    position: relative;
    cursor: pointer;
  }

  .thumbnail {
    width: 100%;
    aspect-ratio: 16 / 10;
    object-fit: cover;
    display: block;
    background: var(--bg-input);
  }

  .placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    color: var(--text-faint);
  }

  .overlay-actions {
    position: absolute;
    top: 6px;
    right: 6px;
    display: flex;
    gap: 4px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .thumb-wrapper:hover .overlay-actions {
    opacity: 1;
  }

  .icon-btn {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: rgba(0, 0, 0, 0.7);
    color: white;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: blur(4px);
    transition: background 0.15s;
    padding: 0;
  }

  .icon-btn:hover {
    background: rgba(74, 158, 255, 0.8);
  }

  .info {
    padding: 8px 10px;
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

  footer {
    padding: 12px 20px;
    border-top: 1px solid var(--border);
    background: var(--bg-footer);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .count {
    font-size: 12px;
    color: var(--text-muted);
  }

  .btn-clear {
    background: var(--danger-bg);
    color: var(--danger);
    border: 1px solid var(--danger-border);
    border-radius: 6px;
    padding: 6px 14px;
    font-size: 12px;
    cursor: pointer;
    font-family: inherit;
    transition: background 0.15s;
  }

  .btn-clear:hover {
    background: var(--danger);
    color: white;
  }

  .preview-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    padding: 24px;
  }

  .preview-container {
    max-width: 90vw;
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    border-radius: 10px;
    overflow: hidden;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
  }

  .preview-img {
    max-width: 100%;
    max-height: calc(90vh - 44px);
    object-fit: contain;
    background: #111;
    display: block;
  }

  .preview-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 12px;
    background: rgba(24, 24, 24, 0.95);
    gap: 12px;
  }

  .preview-name {
    font-size: 12px;
    color: rgba(255, 255, 255, 0.7);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .preview-actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }

  .preview-btn {
    padding: 4px 12px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 5px;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.85);
    font-size: 11px;
    font-family: inherit;
    cursor: pointer;
  }

  .preview-btn:hover {
    background: rgba(255, 255, 255, 0.18);
    color: white;
  }

  .preview-btn.close {
    background: transparent;
    border-color: transparent;
    color: rgba(255, 255, 255, 0.5);
  }

  .preview-btn.close:hover {
    color: white;
  }
</style>
