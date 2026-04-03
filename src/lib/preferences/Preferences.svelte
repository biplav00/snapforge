<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import GeneralTab from "./GeneralTab.svelte";
  import HotkeysTab from "./HotkeysTab.svelte";
  import ScreenshotsTab from "./ScreenshotsTab.svelte";
  import RecordingTab from "./RecordingTab.svelte";

  let activeTab = $state("general");
  let config = $state<Record<string, unknown> | null>(null);
  let saving = $state(false);
  let statusMessage = $state("");
  let statusType = $state<"success" | "error">("success");

  const TABS = [
    { id: "general", label: "General", icon: "⚙" },
    { id: "hotkeys", label: "Hotkeys", icon: "⌨" },
    { id: "screenshots", label: "Screenshots", icon: "📷" },
    { id: "recording", label: "Recording", icon: "🔴" },
  ];

  async function loadConfig() {
    try {
      const json = await invoke<string>("get_config");
      config = JSON.parse(json);
    } catch (e) {
      console.error("Failed to load config:", e);
      statusMessage = `Failed to load: ${e}`;
      statusType = "error";
    }
  }

  function updateConfig(key: string, value: unknown) {
    if (!config) return;
    config = { ...config, [key]: value };
  }

  function updateHotkeyBinding(action: string, binding: string) {
    if (!config) return;
    const bindings = { ...(config.hotkey_bindings as Record<string, string>) };
    bindings[action] = binding;
    config = { ...config, hotkey_bindings: bindings };
  }

  async function saveConfig() {
    if (!config || saving) return;
    saving = true;
    statusMessage = "";
    try {
      await invoke("save_config", { configJson: JSON.stringify(config) });
      await invoke("reload_hotkeys");
      statusMessage = "Settings saved!";
      statusType = "success";
      setTimeout(() => { statusMessage = ""; }, 2500);
    } catch (e) {
      statusMessage = `Error: ${e}`;
      statusType = "error";
    }
    saving = false;
  }

  loadConfig();
</script>

<main>
  <header>
    <h1>ScreenSnap Preferences</h1>
  </header>

  {#if !config}
    <div class="loading">Loading settings...</div>
  {:else}
    <nav class="tabs">
      {#each TABS as tab}
        <button
          class="tab-btn"
          class:active={activeTab === tab.id}
          onclick={() => (activeTab = tab.id)}
        >
          <span class="tab-icon">{tab.icon}</span>
          {tab.label}
        </button>
      {/each}
    </nav>

    <div class="tab-panel">
      {#if activeTab === "general"}
        <GeneralTab {config} onChange={updateConfig} />
      {:else if activeTab === "hotkeys"}
        <HotkeysTab
          bindings={config.hotkey_bindings as { screenshot: string; capture_last_region: string; record_screen: string }}
          onChangeBinding={updateHotkeyBinding}
        />
      {:else if activeTab === "screenshots"}
        <ScreenshotsTab {config} onChange={updateConfig} />
      {:else if activeTab === "recording"}
        <RecordingTab {config} onChange={updateConfig} />
      {/if}
    </div>

    <div class="footer">
      <div class="footer-left">
        {#if statusMessage}
          <span class="status" class:error={statusType === "error"}>
            {statusType === "success" ? "✓" : "✗"} {statusMessage}
          </span>
        {/if}
      </div>
      <button class="save-btn" onclick={saveConfig} disabled={saving}>
        {saving ? "Saving..." : "Save Settings"}
      </button>
    </div>
  {/if}
</main>

<style>
  main {
    padding: 0;
    font-family: system-ui, -apple-system, sans-serif;
    color: #333;
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  header {
    padding: 20px 24px 0;
  }

  h1 {
    font-size: 18px;
    font-weight: 600;
    color: #222;
  }

  .loading {
    padding: 40px 24px;
    color: #888;
    text-align: center;
  }

  .tabs {
    display: flex;
    gap: 0;
    padding: 16px 24px 0;
    border-bottom: 1px solid #e8e8e8;
  }

  .tab-btn {
    padding: 8px 16px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    font-size: 13px;
    cursor: pointer;
    color: #888;
    display: flex;
    align-items: center;
    gap: 6px;
    transition: color 0.15s;
  }

  .tab-btn:hover {
    color: #555;
  }

  .tab-btn.active {
    color: #4a9eff;
    border-bottom-color: #4a9eff;
  }

  .tab-icon {
    font-size: 14px;
  }

  .tab-panel {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px;
  }

  .footer {
    padding: 14px 24px;
    border-top: 1px solid #e8e8e8;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #fafafa;
  }

  .footer-left {
    flex: 1;
  }

  .save-btn {
    padding: 8px 24px;
    background: #4a9eff;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
  }

  .save-btn:hover {
    background: #3a8eef;
  }

  .save-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .status {
    font-size: 13px;
    color: #44aa44;
  }

  .status.error {
    color: #dd4444;
  }
</style>
