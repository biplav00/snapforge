<script lang="ts">
  interface Props {
    config: Record<string, unknown>;
    onChange: (key: string, value: unknown) => void;
  }

  let { config, onChange }: Props = $props();
</script>

<div class="tab-content">
  <div class="field">
    <label for="save-dir">Save Directory</label>
    <input
      id="save-dir"
      type="text"
      value={config.save_directory as string}
      onchange={(e) => onChange("save_directory", (e.target as HTMLInputElement).value)}
    />
  </div>

  <div class="divider"></div>

  <div class="field">
    <label class="toggle">
      <input
        type="checkbox"
        checked={config.auto_copy_clipboard as boolean}
        onchange={(e) => onChange("auto_copy_clipboard", (e.target as HTMLInputElement).checked)}
      />
      <span>Auto-copy to clipboard after capture</span>
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
      <span>Show notification after capture</span>
    </label>
  </div>

  <div class="field">
    <label class="toggle">
      <input
        type="checkbox"
        checked={config.remember_last_region as boolean}
        onchange={(e) => onChange("remember_last_region", (e.target as HTMLInputElement).checked)}
      />
      <span>Remember last region</span>
    </label>
    <span class="hint">Pre-select previous region on next capture</span>
  </div>
</div>

<style>
  .tab-content { display: flex; flex-direction: column; gap: 14px; }
  .field { display: flex; flex-direction: column; gap: 4px; }
  .field > label:not(.toggle) { font-size: 13px; font-weight: 500; color: var(--text-secondary); margin-bottom: 2px; }

  input[type="text"] {
    padding: 7px 10px;
    border: 1px solid var(--border-input);
    border-radius: 6px;
    font-size: 13px;
    font-family: monospace;
    background: var(--bg-input);
    color: var(--text);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); background: var(--bg-input-focus); }

  .toggle { display: flex; align-items: center; gap: 10px; cursor: pointer; font-size: 14px; color: var(--text); }
  input[type="checkbox"] { width: 16px; height: 16px; accent-color: var(--accent); }
  .hint { font-size: 12px; color: var(--text-muted); margin-left: 26px; }
  .divider { height: 1px; background: var(--border-light); margin: 4px 0; }
</style>
