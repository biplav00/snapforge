<script lang="ts">
  interface Props {
    config: Record<string, unknown>;
    onChange: (key: string, value: unknown) => void;
  }

  let { config, onChange }: Props = $props();

  const FORMATS = [
    { value: "Png", label: "PNG", desc: "Lossless, best quality" },
    { value: "Jpg", label: "JPG", desc: "Smaller files, lossy" },
    { value: "WebP", label: "WebP", desc: "Modern, good compression" },
  ];
</script>

<div class="tab-content">
  <div class="field">
    <label>Default Format</label>
    <div class="format-group">
      {#each FORMATS as fmt}
        <label class="format-option" class:selected={config.screenshot_format === fmt.value}>
          <input
            type="radio"
            name="format"
            value={fmt.value}
            checked={config.screenshot_format === fmt.value}
            onchange={() => onChange("screenshot_format", fmt.value)}
          />
          <div class="format-content">
            <span class="format-name">{fmt.label}</span>
            <span class="format-desc">{fmt.desc}</span>
          </div>
        </label>
      {/each}
    </div>
  </div>

  <div class="field">
    <label for="quality">Quality: <strong>{config.jpg_quality}</strong></label>
    <input
      id="quality"
      type="range"
      min="1"
      max="100"
      value={config.jpg_quality as number}
      oninput={(e) => onChange("jpg_quality", parseInt((e.target as HTMLInputElement).value))}
    />
    <div class="range-labels">
      <span>Smaller file</span>
      <span>Better quality</span>
    </div>
    <span class="hint">Applies to JPG and WebP formats</span>
  </div>

  <div class="field">
    <label for="pattern">Filename Pattern</label>
    <input
      id="pattern"
      type="text"
      value={config.filename_pattern as string}
      onchange={(e) => onChange("filename_pattern", (e.target as HTMLInputElement).value)}
    />
    <span class="hint">Use {"{date}"} for date and {"{time}"} for time</span>
  </div>
</div>

<style>
  .tab-content {
    display: flex;
    flex-direction: column;
    gap: 18px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .field > label {
    font-size: 13px;
    font-weight: 500;
    color: #555;
  }

  .format-group {
    display: flex;
    gap: 8px;
  }

  .format-option {
    flex: 1;
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
    border: 1px solid #ddd;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .format-option:hover {
    border-color: #bbb;
    background: #fafafa;
  }

  .format-option.selected {
    border-color: #4a9eff;
    background: #f0f6ff;
  }

  .format-option input[type="radio"] {
    margin-top: 2px;
    accent-color: #4a9eff;
  }

  .format-content {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .format-name {
    font-size: 14px;
    font-weight: 600;
  }

  .format-desc {
    font-size: 11px;
    color: #888;
  }

  input[type="text"] {
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

  input[type="range"] {
    width: 100%;
    accent-color: #4a9eff;
  }

  .range-labels {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    color: #aaa;
  }

  .hint {
    font-size: 12px;
    color: #999;
  }
</style>
