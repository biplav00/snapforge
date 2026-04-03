<script lang="ts">
  interface HotkeyBindings {
    screenshot: string;
    capture_last_region: string;
  }

  interface Props {
    bindings: HotkeyBindings;
    onChangeBinding: (action: string, newBinding: string) => void;
  }

  let { bindings, onChangeBinding }: Props = $props();

  let recording = $state<string | null>(null);

  const ACTIONS = [
    { id: "screenshot", label: "Screenshot" },
    { id: "capture_last_region", label: "Capture Last Region" },
  ];

  function startRecording(action: string) {
    recording = action;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();

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
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="tab-content">
  <table class="hotkey-table">
    <thead>
      <tr>
        <th>Action</th>
        <th>Shortcut</th>
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each ACTIONS as action}
        <tr>
          <td>{action.label}</td>
          <td class="binding">
            {#if recording === action.id}
              <span class="recording">Press keys...</span>
            {:else}
              <code>{bindings[action.id as keyof HotkeyBindings]}</code>
            {/if}
          </td>
          <td>
            <button class="rebind-btn" onclick={() => startRecording(action.id)}>
              {recording === action.id ? "..." : "Rebind"}
            </button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>

  <button class="reset-btn" onclick={resetDefaults}>Reset to Defaults</button>
</div>

<style>
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .hotkey-table {
    width: 100%;
    border-collapse: collapse;
  }

  .hotkey-table th {
    text-align: left;
    padding: 8px;
    border-bottom: 1px solid #ddd;
    font-size: 13px;
    color: #666;
  }

  .hotkey-table td {
    padding: 8px;
    border-bottom: 1px solid #eee;
    font-size: 14px;
  }

  .binding code {
    background: #f0f0f0;
    padding: 2px 8px;
    border-radius: 4px;
    font-size: 13px;
  }

  .recording {
    color: #4a9eff;
    font-style: italic;
  }

  .rebind-btn {
    padding: 4px 12px;
    border: 1px solid #ccc;
    border-radius: 4px;
    background: white;
    cursor: pointer;
    font-size: 12px;
  }

  .rebind-btn:hover {
    background: #f0f0f0;
  }

  .reset-btn {
    align-self: flex-start;
    padding: 6px 16px;
    border: 1px solid #ccc;
    border-radius: 6px;
    background: white;
    cursor: pointer;
    font-size: 13px;
  }

  .reset-btn:hover {
    background: #f0f0f0;
  }
</style>
