<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

  let elapsed = $state(0);
  let interval: ReturnType<typeof setInterval> | null = null;

  const appWindow = getCurrentWebviewWindow();

  function formatTime(seconds: number): string {
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }

  async function stopRecording() {
    try {
      await invoke("stop_recording");
    } catch (e) {
      console.error("Failed to stop:", e);
    }
    if (interval) clearInterval(interval);
    appWindow.close();
  }

  interval = setInterval(() => {
    elapsed += 1;
  }, 1000);
</script>

<div class="indicator">
  <div class="dot"></div>
  <span class="time">{formatTime(elapsed)}</span>
  <button class="stop-btn" onclick={stopRecording}>■ Stop</button>
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
    cursor: default;
    user-select: none;
    -webkit-app-region: drag;
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
    -webkit-app-region: no-drag;
  }

  .stop-btn:hover {
    background: rgba(255, 60, 60, 0.4);
    color: white;
  }
</style>
