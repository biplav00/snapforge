<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import GeneralTab from "./GeneralTab.svelte";
  import HotkeysTab from "./HotkeysTab.svelte";
  import ScreenshotsTab from "./ScreenshotsTab.svelte";

  let activeTab = $state("general");
  let config = $state<Record<string, unknown> | null>(null);
  let saving = $state(false);
  let statusMessage = $state("");

  const TABS = [
    { id: "general", label: "General" },
    { id: "hotkeys", label: "Hotkeys" },
    { id: "screenshots", label: "Screenshots" },
  ];

  async function loadConfig() {
    try {
      const json = await invoke<string>("get_config");
      config = JSON.parse(json);
    } catch (e) {
      console.error("Failed to load config:", e);
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
      setTimeout(() => { statusMessage = ""; }, 2000);
    } catch (e) {
      statusMessage = `Error: ${e}`;
    }
    saving = false;
  }

  loadConfig();
</script>

<main>
  <h1>Preferences</h1>

  {#if !config}
    <p>Loading...</p>
  {:else}
    <nav class="tabs">
      {#each TABS as tab}
        <button
          class="tab-btn"
          class:active={activeTab === tab.id}
          onclick={() => (activeTab = tab.id)}
        >
          {tab.label}
        </button>
      {/each}
    </nav>

    <div class="tab-panel">
      {#if activeTab === "general"}
        <GeneralTab {config} onChange={updateConfig} />
      {:else if activeTab === "hotkeys"}
        <HotkeysTab
          bindings={config.hotkey_bindings as { screenshot: string; capture_last_region: string }}
          onChangeBinding={updateHotkeyBinding}
        />
      {:else if activeTab === "screenshots"}
        <ScreenshotsTab {config} onChange={updateConfig} />
      {/if}
    </div>

    <div class="footer">
      <button class="save-btn" onclick={saveConfig} disabled={saving}>
        {saving ? "Saving..." : "Save"}
      </button>
      {#if statusMessage}
        <span class="status">{statusMessage}</span>
      {/if}
    </div>
  {/if}
</main>

<style>
  main {
    padding: 24px;
    font-family: system-ui, sans-serif;
    color: #333;
    max-width: 560px;
  }

  h1 {
    font-size: 20px;
    margin-bottom: 16px;
  }

  .tabs {
    display: flex;
    gap: 0;
    border-bottom: 1px solid #ddd;
    margin-bottom: 20px;
  }

  .tab-btn {
    padding: 8px 20px;
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    font-size: 14px;
    cursor: pointer;
    color: #666;
  }

  .tab-btn:hover {
    color: #333;
  }

  .tab-btn.active {
    color: #4a9eff;
    border-bottom-color: #4a9eff;
  }

  .tab-panel {
    min-height: 200px;
  }

  .footer {
    margin-top: 24px;
    padding-top: 16px;
    border-top: 1px solid #eee;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .save-btn {
    padding: 8px 24px;
    background: #4a9eff;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 14px;
    cursor: pointer;
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
</style>
