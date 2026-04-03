<script lang="ts">
  interface HotkeyBindings {
    screenshot: string;
    capture_last_region: string;
    record_screen: string;
  }

  interface Props {
    bindings: HotkeyBindings;
    onChangeBinding: (action: string, newBinding: string) => void;
  }

  let { bindings, onChangeBinding }: Props = $props();

  let recording = $state<string | null>(null);

  const ACTIONS = [
    { id: "screenshot", label: "Screenshot", description: "Open capture overlay" },
    { id: "capture_last_region", label: "Capture Last Region", description: "Re-capture previous region" },
    { id: "record_screen", label: "Record Screen", description: "Start/stop screen recording" },
  ];

  function startRecording(action: string) {
    recording = action;
  }

  function cancelRecording() {
    recording = null;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

    if (e.key === "Escape") {
      recording = null;
      return;
    }

    if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;

    const parts: string[] = [];
    if (e.ctrlKey || e.metaKey) parts.push("CmdOrCtrl");
    if (e.shiftKey) parts.push("Shift");
    if (e.altKey) parts.push("Alt");

    let key = e.key.length === 1 ? e.key.toUpperCase() : e.code;
    if (key.startsWith("Key")) key = key.slice(3);
    if (key.startsWith("Digit")) key = key.slice(5);

    parts.push(key);
    const combo = parts.join("+");

    onChangeBinding(recording, combo);
    recording = null;
  }

  function resetDefaults() {
    onChangeBinding("screenshot", "CmdOrCtrl+Shift+S");
    onChangeBinding("capture_last_region", "CmdOrCtrl+Shift+L");
    onChangeBinding("record_screen", "CmdOrCtrl+Shift+R");
  }

  function formatShortcut(shortcut: string): string {
    return shortcut
      .replace("CmdOrCtrl", "⌘")
      .replace("Shift", "⇧")
      .replace("Alt", "⌥")
      .replace(/\+/g, " ");
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="tab-content">
  <div class="hotkey-list">
    {#each ACTIONS as action}
      <div class="hotkey-row" class:active={recording === action.id}>
        <div class="hotkey-info">
          <span class="hotkey-label">{action.label}</span>
          <span class="hotkey-desc">{action.description}</span>
        </div>
        <div class="hotkey-binding">
          {#if recording === action.id}
            <span class="recording-indicator">Press shortcut...</span>
            <button class="cancel-btn" onclick={cancelRecording}>Cancel</button>
          {:else}
            <kbd class="shortcut">{formatShortcut(bindings[action.id as keyof HotkeyBindings])}</kbd>
            <button class="rebind-btn" onclick={() => startRecording(action.id)}>Change</button>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <button class="reset-btn" onclick={resetDefaults}>Reset All to Defaults</button>
</div>

<style>
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .hotkey-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .hotkey-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 12px;
    border-radius: 8px;
    transition: background 0.15s;
  }

  .hotkey-row:hover {
    background: #f5f7fa;
  }

  .hotkey-row.active {
    background: #eef4ff;
  }

  .hotkey-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .hotkey-label {
    font-size: 14px;
    font-weight: 500;
  }

  .hotkey-desc {
    font-size: 12px;
    color: #888;
  }

  .hotkey-binding {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .shortcut {
    background: #f0f0f0;
    border: 1px solid #ddd;
    padding: 3px 10px;
    border-radius: 5px;
    font-size: 13px;
    font-family: system-ui, sans-serif;
    letter-spacing: 1px;
    min-width: 60px;
    text-align: center;
  }

  .recording-indicator {
    color: #4a9eff;
    font-size: 13px;
    font-style: italic;
    animation: blink 1s ease-in-out infinite;
  }

  @keyframes blink {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .rebind-btn, .cancel-btn {
    padding: 4px 12px;
    border: 1px solid #d0d0d0;
    border-radius: 5px;
    background: white;
    cursor: pointer;
    font-size: 12px;
    color: #555;
  }

  .rebind-btn:hover { background: #f0f0f0; }
  .cancel-btn { border-color: #ffaaaa; color: #cc4444; }
  .cancel-btn:hover { background: #fff0f0; }

  .reset-btn {
    align-self: flex-start;
    padding: 6px 16px;
    border: 1px solid #d0d0d0;
    border-radius: 6px;
    background: white;
    cursor: pointer;
    font-size: 13px;
    color: #666;
  }

  .reset-btn:hover {
    background: #f0f0f0;
  }
</style>
