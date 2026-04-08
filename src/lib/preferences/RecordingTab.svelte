<script lang="ts">
interface Props {
  config: Record<string, unknown>;
  onChange: (key: string, value: unknown) => void;
}

let { config, onChange }: Props = $props();
let recording = $derived(config.recording as Record<string, unknown>);

function updateRecording(key: string, value: unknown) {
  onChange("recording", { ...recording, [key]: value });
}

const FORMATS = [
  { value: "Mp4", label: "MP4", desc: "H.264, universal playback" },
  { value: "Gif", label: "GIF", desc: "Animated, larger files" },
];

const FPS_OPTIONS = [
  { value: 10, label: "10" },
  { value: 15, label: "15" },
  { value: 24, label: "24" },
  { value: 30, label: "30" },
  { value: 60, label: "60" },
];

const QUALITY_OPTIONS = [
  { value: "Low", label: "Low" },
  { value: "Medium", label: "Medium" },
  { value: "High", label: "High" },
];
</script>

<div class="tab-content">
  <div class="field" role="group" aria-label="Format">
    <span class="field-label">Format</span>
    <div class="format-group">
      {#each FORMATS as fmt}
        <label class="format-option" class:selected={recording.format === fmt.value}>
          <input type="radio" name="rec-format" value={fmt.value}
            checked={recording.format === fmt.value}
            onchange={() => updateRecording("format", fmt.value)} />
          <span class="radio-dot"></span>
          <div class="format-content">
            <span class="format-name">{fmt.label}</span>
            <span class="format-desc">{fmt.desc}</span>
          </div>
        </label>
      {/each}
    </div>
  </div>

  <div class="field" role="group" aria-label="Frame Rate (fps)">
    <span class="field-label">Frame Rate (fps)</span>
    <div class="pill-group">
      {#each FPS_OPTIONS as opt}
        <label class="pill" class:selected={recording.fps === opt.value}>
          <input type="radio" name="fps" value={opt.value}
            checked={recording.fps === opt.value}
            onchange={() => updateRecording("fps", opt.value)} />
          {opt.label}
        </label>
      {/each}
    </div>
  </div>

  <div class="field" role="group" aria-label="Quality">
    <span class="field-label">Quality</span>
    <div class="pill-group">
      {#each QUALITY_OPTIONS as opt}
        <label class="pill" class:selected={recording.quality === opt.value}>
          <input type="radio" name="quality" value={opt.value}
            checked={recording.quality === opt.value}
            onchange={() => updateRecording("quality", opt.value)} />
          {opt.label}
        </label>
      {/each}
    </div>
  </div>
</div>

<style>
  .tab-content { display: flex; flex-direction: column; gap: 18px; }
  .field { display: flex; flex-direction: column; gap: 6px; }
  .field > .field-label { font-size: 13px; font-weight: 500; color: var(--text-secondary); }

  /* Custom radio card group */
  .format-group { display: flex; gap: 8px; }
  .format-option {
    flex: 1; display: flex; align-items: flex-start; gap: 10px;
    padding: 10px 12px; border: 1px solid var(--border); border-radius: 8px;
    cursor: pointer; transition: all 0.15s; background: var(--bg);
  }
  .format-option:hover { border-color: var(--border-input); background: var(--bg-hover); }
  .format-option.selected { border-color: var(--accent); background: var(--accent-bg); }

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
  .format-option.selected .radio-dot { border-color: var(--accent); }
  .format-option.selected .radio-dot::after { background: var(--accent); }

  .format-content { display: flex; flex-direction: column; gap: 2px; }
  .format-name { font-size: 14px; font-weight: 600; color: var(--text); }
  .format-desc { font-size: 11px; color: var(--text-muted); }

  /* Pill group */
  .pill-group { display: flex; gap: 6px; flex-wrap: wrap; }
  .pill {
    display: flex; align-items: center;
    padding: 6px 16px; border: 1px solid var(--border); border-radius: 20px;
    font-size: 13px; cursor: pointer; transition: all 0.15s;
    background: var(--bg); color: var(--text);
  }
  .pill:hover { border-color: var(--border-input); background: var(--bg-hover); }
  .pill.selected { border-color: var(--accent); background: var(--accent-bg); color: var(--accent); font-weight: 500; }
  .pill input[type="radio"] { display: none; }
</style>
