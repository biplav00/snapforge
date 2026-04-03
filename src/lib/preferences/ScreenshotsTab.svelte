<script lang="ts">
  interface Props {
    config: Record<string, unknown>;
    onChange: (key: string, value: unknown) => void;
  }

  let { config, onChange }: Props = $props();

  const FORMATS = [
    { value: "Png", label: "PNG" },
    { value: "Jpg", label: "JPG" },
    { value: "WebP", label: "WebP" },
  ];
</script>

<div class="tab-content">
  <div class="field">
    <label>Default Format</label>
    <div class="radio-group">
      {#each FORMATS as fmt}
        <label class="radio-label">
          <input
            type="radio"
            name="format"
            value={fmt.value}
            checked={config.screenshot_format === fmt.value}
            onchange={() => onChange("screenshot_format", fmt.value)}
          />
          {fmt.label}
        </label>
      {/each}
    </div>
  </div>

  <div class="field">
    <label>JPG/WebP Quality: {config.jpg_quality}</label>
    <input
      type="range"
      min="1"
      max="100"
      value={config.jpg_quality as number}
      oninput={(e) => onChange("jpg_quality", parseInt((e.target as HTMLInputElement).value))}
    />
  </div>

  <div class="field">
    <label>Filename Pattern</label>
    <input
      type="text"
      value={config.filename_pattern as string}
      onchange={(e) => onChange("filename_pattern", (e.target as HTMLInputElement).value)}
    />
    <small class="hint">Use {"{date}"} and {"{time}"} as placeholders</small>
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
  }

  .radio-group {
    display: flex;
    gap: 16px;
  }

  .radio-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 14px;
    cursor: pointer;
  }

  input[type="text"] {
    padding: 6px 10px;
    border: 1px solid #ccc;
    border-radius: 6px;
    font-size: 14px;
    font-family: monospace;
  }

  input[type="range"] {
    width: 100%;
  }

  .hint {
    color: #888;
    font-size: 12px;
  }
</style>
