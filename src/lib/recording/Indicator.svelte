<script lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { onDestroy } from "svelte";

let elapsed = $state(0);
let stopping = $state(false);

const appWindow = getCurrentWebviewWindow();

function formatTime(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

function handleDragStart(e: MouseEvent) {
  // Don't drag when clicking the stop button
  if ((e.target as HTMLElement).closest("button")) return;
  // Use Tauri's native window drag
  appWindow.startDragging();
}

async function stopRecording() {
  if (stopping) return;
  stopping = true;
  try {
    const path = await invoke<string>("stop_recording");
    if (path) {
      // Try copying to clipboard — works for GIF, may fail for MP4 (that's OK)
      try {
        await invoke("copy_file_to_clipboard", { path });
        await invoke("show_toast", { message: "Recording saved & copied!" });
      } catch {
        await invoke("show_toast", { message: "Recording saved!" });
      }
    } else {
      await invoke("show_toast", { message: "Recording saved!" });
    }
  } catch (e) {
    console.error("Failed to stop:", e);
  }
  appWindow.close();
}

const interval = setInterval(() => {
  elapsed += 1;
}, 1000);

onDestroy(() => clearInterval(interval));
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="indicator" onmousedown={handleDragStart}>
  <div class="dot"></div>
  <span class="time">{formatTime(elapsed)}</span>
  <button class="stop-btn" onclick={stopRecording} disabled={stopping}>
    {stopping ? "Saving..." : "■ Stop"}
  </button>
</div>

<style>
  .indicator {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(20, 20, 20, 0.9);
    backdrop-filter: blur(8px);
    border: 1px solid rgba(255, 60, 60, 0.4);
    border-radius: 20px;
    padding: 6px 14px;
    font-family: system-ui, sans-serif;
    cursor: grab;
    user-select: none;
  }

  .indicator:active {
    cursor: grabbing;
  }

  .dot {
    width: 10px;
    height: 10px;
    background: #ff3333;
    border-radius: 50%;
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .time {
    color: white;
    font-size: 14px;
    font-variant-numeric: tabular-nums;
    min-width: 40px;
  }

  .stop-btn {
    background: rgba(255, 60, 60, 0.2);
    color: #ff6666;
    border: 1px solid rgba(255, 60, 60, 0.3);
    border-radius: 12px;
    padding: 3px 10px;
    font-size: 12px;
    cursor: pointer;
    font-family: system-ui, sans-serif;
  }

  .stop-btn:hover {
    background: rgba(255, 60, 60, 0.4);
    color: white;
  }

  .stop-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
