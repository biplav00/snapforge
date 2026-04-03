<script lang="ts">
  interface Props {
    config: Record<string, unknown>;
    onChange: (key: string, value: unknown) => void;
  }

  let { config, onChange }: Props = $props();
</script>

<div class="tab-content">
  <div class="field">
    <label>Save Directory</label>
    <input
      type="text"
      value={config.save_directory as string}
      onchange={(e) => onChange("save_directory", (e.target as HTMLInputElement).value)}
    />
  </div>

  <div class="field">
    <label>
      <input
        type="checkbox"
        checked={config.auto_copy_clipboard as boolean}
        onchange={(e) => onChange("auto_copy_clipboard", (e.target as HTMLInputElement).checked)}
      />
      Auto-copy to clipboard after capture
    </label>
  </div>

  <div class="field">
    <label>
      <input
        type="checkbox"
        checked={config.show_notification as boolean}
        onchange={(e) => onChange("show_notification", (e.target as HTMLInputElement).checked)}
      />
      Show notification after capture
    </label>
  </div>

  <div class="field">
    <label>
      <input
        type="checkbox"
        checked={config.remember_last_region as boolean}
        onchange={(e) => onChange("remember_last_region", (e.target as HTMLInputElement).checked)}
      />
      Remember last region
    </label>
  </div>
</div>

<style>
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field label {
    font-size: 14px;
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
  }

  input[type="text"] {
    padding: 6px 10px;
    border: 1px solid #ccc;
    border-radius: 6px;
    font-size: 14px;
    font-family: monospace;
  }

  input[type="checkbox"] {
    width: 16px;
    height: 16px;
  }
</style>
