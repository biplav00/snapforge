<script lang="ts">
interface Props {
  config: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}

let { config, onChange }: Props = $props();

let dirError = $state("");

function validateAndSetDir(value: string) {
  const trimmed = value.trim();
  if (!trimmed) {
    dirError = "Directory cannot be empty";
    return;
  }
  if (!trimmed.startsWith("/") && !trimmed.match(/^[A-Z]:\\/i) && !trimmed.startsWith("~")) {
    dirError = "Must be an absolute path";
    return;
  }
  dirError = "";
  onChange("save_directory", trimmed);
}
</script>

<div class="tab-content">
  <div class="field">
    <label for="save-dir">Save Directory</label>
    <input
      id="save-dir"
      type="text"
      class:invalid={!!dirError}
      value={config.save_directory as string}
      onchange={(e) => validateAndSetDir((e.target as HTMLInputElement).value)}
    />
    {#if dirError}
      <span class="field-error">{dirError}</span>
    {/if}
  </div>

  <div class="divider"></div>

  <label class="switch-row">
    <span class="switch-label">Auto-copy to clipboard after capture</span>
    <span class="switch">
      <input type="checkbox"
        checked={config.auto_copy_clipboard as boolean}
        onchange={(e) => onChange("auto_copy_clipboard", (e.target as HTMLInputElement).checked)} />
      <span class="slider"></span>
    </span>
  </label>

  <label class="switch-row">
    <span class="switch-label">Show notification after capture</span>
    <span class="switch">
      <input type="checkbox"
        checked={config.show_notification as boolean}
        onchange={(e) => onChange("show_notification", (e.target as HTMLInputElement).checked)} />
      <span class="slider"></span>
    </span>
  </label>

  <label class="switch-row">
    <span class="switch-label">Remember last region</span>
    <span class="switch">
      <input type="checkbox"
        checked={config.remember_last_region as boolean}
        onchange={(e) => onChange("remember_last_region", (e.target as HTMLInputElement).checked)} />
      <span class="slider"></span>
    </span>
  </label>
</div>

<style>
  .tab-content { display: flex; flex-direction: column; gap: 14px; }
  .field { display: flex; flex-direction: column; gap: 4px; }
  .field > label { font-size: 13px; font-weight: 500; color: var(--text-secondary); margin-bottom: 2px; }

  input[type="text"] {
    padding: 7px 10px; border: 1px solid var(--border-input); border-radius: 6px;
    font-size: 13px; font-family: monospace; background: var(--bg-input); color: var(--text);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); background: var(--bg-input-focus); }
  input[type="text"].invalid { border-color: #ff4444; }

  .field-error { font-size: 11px; color: #ff4444; margin-top: 2px; }

  .divider { height: 1px; background: var(--border-light); margin: 4px 0; }

  /* Toggle switch rows */
  .switch-row {
    display: flex; align-items: center; justify-content: space-between;
    padding: 6px 0; cursor: pointer;
  }
  .switch-label { font-size: 14px; color: var(--text); }

  /* Custom toggle switch */
  .switch {
    position: relative; width: 40px; height: 22px; flex-shrink: 0;
  }
  .switch input {
    opacity: 0; width: 0; height: 0; position: absolute;
  }
  .slider {
    position: absolute; inset: 0;
    background: var(--border-input); border-radius: 22px;
    transition: background 0.2s;
  }
  .slider::before {
    content: ""; position: absolute;
    width: 16px; height: 16px; left: 3px; bottom: 3px;
    background: white; border-radius: 50%;
    transition: transform 0.2s;
    box-shadow: 0 1px 3px rgba(0,0,0,0.15);
  }
  .switch input:checked + .slider {
    background: var(--accent);
  }
  .switch input:checked + .slider::before {
    transform: translateX(18px);
  }
</style>
