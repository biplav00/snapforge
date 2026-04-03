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
          <input type="radio" name="format" value={fmt.value}
            checked={config.screenshot_format === fmt.value}
            onchange={() => onChange("screenshot_format", fmt.value)} />
          <span class="radio-dot"></span>
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
    <div class="range-wrapper">
      <input id="quality" type="range" min="1" max="100"
        value={config.jpg_quality as number}
        oninput={(e) => onChange("jpg_quality", parseInt((e.target as HTMLInputElement).value))} />
    </div>
    <div class="range-labels"><span>Smaller file</span><span>Better quality</span></div>
    <span class="hint">Applies to JPG and WebP formats</span>
  </div>

  <div class="field">
    <label for="pattern">Filename Pattern</label>
    <input id="pattern" type="text" value={config.filename_pattern as string}
      onchange={(e) => onChange("filename_pattern", (e.target as HTMLInputElement).value)} />
    <span class="hint">Use {"{date}"} for date and {"{time}"} for time</span>
  </div>
</div>

<style>
  .tab-content { display: flex; flex-direction: column; gap: 18px; }
  .field { display: flex; flex-direction: column; gap: 6px; }
  .field > label { font-size: 13px; font-weight: 500; color: var(--text-secondary); }

  /* Custom radio card group */
  .format-group { display: flex; gap: 8px; }
  .format-option {
    flex: 1; display: flex; align-items: flex-start; gap: 10px;
    padding: 10px 12px; border: 1px solid var(--border); border-radius: 8px;
    cursor: pointer; transition: all 0.15s; background: var(--bg);
  }
  .format-option:hover { border-color: var(--border-input); background: var(--bg-hover); }
  .format-option.selected { border-color: var(--accent); background: var(--accent-bg); }

  /* Hide native radio, show custom dot */
  .format-option input[type="radio"] { position: absolute; opacity: 0; width: 0; height: 0; }
  .radio-dot {
    width: 16px; height: 16px; border-radius: 50%; flex-shrink: 0; margin-top: 1px;
    border: 2px solid var(--border-input); transition: all 0.15s;
    position: relative;
  }
  .radio-dot::after {
    content: ""; position: absolute;
    width: 8px; height: 8px; border-radius: 50%;
    top: 2px; left: 2px;
    background: transparent; transition: background 0.15s;
  }
  .format-option.selected .radio-dot {
    border-color: var(--accent);
  }
  .format-option.selected .radio-dot::after {
    background: var(--accent);
  }

  .format-content { display: flex; flex-direction: column; gap: 2px; }
  .format-name { font-size: 14px; font-weight: 600; color: var(--text); }
  .format-desc { font-size: 11px; color: var(--text-muted); }

  /* Text input */
  input[type="text"] {
    padding: 7px 10px; border: 1px solid var(--border-input); border-radius: 6px;
    font-size: 13px; font-family: monospace; background: var(--bg-input); color: var(--text);
  }
  input[type="text"]:focus { outline: none; border-color: var(--accent); background: var(--bg-input-focus); }

  /* Custom range slider */
  .range-wrapper { padding: 4px 0; }

  input[type="range"] {
    -webkit-appearance: none; appearance: none;
    width: 100%; height: 6px; border-radius: 3px;
    background: var(--border-light); outline: none;
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none; appearance: none;
    width: 18px; height: 18px; border-radius: 50%;
    background: var(--accent); cursor: pointer;
    box-shadow: 0 1px 4px rgba(0,0,0,0.2);
    border: 2px solid white;
    transition: transform 0.1s;
  }
  input[type="range"]::-webkit-slider-thumb:hover {
    transform: scale(1.15);
  }
  input[type="range"]::-webkit-slider-thumb:active {
    transform: scale(1.05);
  }
  input[type="range"]::-moz-range-thumb {
    width: 18px; height: 18px; border-radius: 50%;
    background: var(--accent); cursor: pointer;
    box-shadow: 0 1px 4px rgba(0,0,0,0.2);
    border: 2px solid white;
  }
  input[type="range"]::-moz-range-track {
    height: 6px; border-radius: 3px; background: var(--border-light);
  }

  .range-labels { display: flex; justify-content: space-between; font-size: 11px; color: var(--text-faint); }
  .hint { font-size: 12px; color: var(--text-muted); }
</style>
