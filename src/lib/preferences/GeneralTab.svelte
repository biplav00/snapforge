<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface Props {
    config: Record<string, unknown>;
    onChange: (key: string, value: unknown) => void;
  }

  let { config, onChange }: Props = $props();

  async function browseSaveDir() {
    try {
      const result = await invoke<string>("open_save_folder");
    } catch (e) {
      // open_save_folder opens the folder, not a picker
      // For now just let user type the path
    }
  }
</script>

<div class="tab-content">
  <div class="field">
    <label for="save-dir">Save Directory</label>
    <div class="input-row">
      <input
        id="save-dir"
        type="text"
        value={config.save_directory as string}
        onchange={(e) => onChange("save_directory", (e.target as HTMLInputElement).value)}
      />
    </div>
  </div>

  <div class="divider"></div>

  <div class="field">
    <label class="toggle">
      <input
        type="checkbox"
        checked={config.auto_copy_clipboard as boolean}
        onchange={(e) => onChange("auto_copy_clipboard", (e.target as HTMLInputElement).checked)}
      />
      <span class="toggle-label">Auto-copy to clipboard after capture</span>
    </label>
    <span class="hint">Automatically copy screenshot to clipboard when saving</span>
  </div>

  <div class="field">
    <label class="toggle">
      <input
        type="checkbox"
        checked={config.show_notification as boolean}
        onchange={(e) => onChange("show_notification", (e.target as HTMLInputElement).checked)}
      />
      <span class="toggle-label">Show notification after capture</span>
    </label>
  </div>

  <div class="field">
    <label class="toggle">
      <input
        type="checkbox"
        checked={config.remember_last_region as boolean}
        onchange={(e) => onChange("remember_last_region", (e.target as HTMLInputElement).checked)}
      />
      <span class="toggle-label">Remember last region</span>
    </label>
    <span class="hint">Pre-select previous region on next capture</span>
  </div>
</div>

<style>
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field > label:not(.toggle) {
    font-size: 13px;
    font-weight: 500;
    color: #555;
    margin-bottom: 2px;
  }

  .input-row {
    display: flex;
    gap: 8px;
  }

  input[type="text"] {
    flex: 1;
    padding: 7px 10px;
    border: 1px solid #d0d0d0;
    border-radius: 6px;
    font-size: 13px;
    font-family: monospace;
    background: #fafafa;
  }

  input[type="text"]:focus {
    outline: none;
    border-color: #4a9eff;
    background: white;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    font-size: 14px;
  }

  .toggle-label {
    font-weight: 400;
  }

  input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: #4a9eff;
  }

  .hint {
    font-size: 12px;
    color: #999;
    margin-left: 26px;
  }

  .divider {
    height: 1px;
    background: #eee;
    margin: 4px 0;
  }
</style>
