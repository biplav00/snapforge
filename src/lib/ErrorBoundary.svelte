<script lang="ts">
import type { Snippet } from "svelte";

interface Props {
  children: Snippet;
}

let { children }: Props = $props();
let error = $state<Error | null>(null);

function handleError(e: ErrorEvent) {
  error = e.error instanceof Error ? e.error : new Error(String(e.error));
  e.preventDefault();
}

function handleUnhandledRejection(e: PromiseRejectionEvent) {
  error = e.reason instanceof Error ? e.reason : new Error(String(e.reason));
  e.preventDefault();
}

function dismiss() {
  error = null;
}
</script>

<svelte:window onerror={handleError} onunhandledrejection={handleUnhandledRejection} />

{#if error}
  <div class="error-overlay">
    <div class="error-dialog">
      <h3>Something went wrong</h3>
      <pre class="error-message">{error.message}</pre>
      <div class="error-actions">
        <button class="btn-dismiss" onclick={dismiss}>Dismiss</button>
        <button class="btn-reload" onclick={() => location.reload()}>Reload</button>
      </div>
    </div>
  </div>
{/if}

{@render children()}

<style>
  .error-overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
  }

  .error-dialog {
    background: #1e1e1e;
    border: 1px solid rgba(255, 80, 80, 0.4);
    border-radius: 10px;
    padding: 20px 24px;
    max-width: 420px;
    width: 90%;
    font-family: system-ui, -apple-system, sans-serif;
  }

  h3 {
    color: #ff6666;
    margin: 0 0 12px;
    font-size: 15px;
    font-weight: 600;
  }

  .error-message {
    color: rgba(255, 255, 255, 0.7);
    font-size: 12px;
    font-family: monospace;
    background: rgba(0, 0, 0, 0.3);
    padding: 8px 10px;
    border-radius: 6px;
    margin: 0 0 16px;
    overflow-x: auto;
    max-height: 120px;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .error-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .btn-dismiss {
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.7);
    border: none;
    border-radius: 5px;
    padding: 6px 14px;
    font-size: 12px;
    cursor: pointer;
  }
  .btn-dismiss:hover { background: rgba(255, 255, 255, 0.2); color: white; }

  .btn-reload {
    background: #4a9eff;
    color: white;
    border: none;
    border-radius: 5px;
    padding: 6px 14px;
    font-size: 12px;
    cursor: pointer;
  }
  .btn-reload:hover { background: #3a8eef; }
</style>
