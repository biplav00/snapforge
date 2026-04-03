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
    { value: 10, label: "10 fps", desc: "Smallest files" },
    { value: 15, label: "15 fps", desc: "Good for tutorials" },
    { value: 24, label: "24 fps", desc: "Film-like" },
    { value: 30, label: "30 fps", desc: "Smooth (default)" },
    { value: 60, label: "60 fps", desc: "Very smooth, large files" },
  ];

  const QUALITY_OPTIONS = [
    { value: "Low", label: "Low", desc: "Smallest files" },
    { value: "Medium", label: "Medium", desc: "Balanced (default)" },
    { value: "High", label: "High", desc: "Best quality" },
  ];
</script>

<div class="tab-content">
  <div class="field">
    <label>Format</label>
    <div class="format-group">
      {#each FORMATS as fmt}
        <label class="format-option" class:selected={recording.format === fmt.value}>
          <input
            type="radio"
            name="rec-format"
            value={fmt.value}
            checked={recording.format === fmt.value}
            onchange={() => updateRecording("format", fmt.value)}
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
    <label>Frame Rate</label>
    <div class="option-group">
      {#each FPS_OPTIONS as opt}
        <label class="pill" class:selected={recording.fps === opt.value}>
          <input
            type="radio"
            name="fps"
            value={opt.value}
            checked={recording.fps === opt.value}
            onchange={() => updateRecording("fps", opt.value)}
          />
          {opt.label}
        </label>
      {/each}
    </div>
  </div>

  <div class="field">
    <label>Quality</label>
    <div class="option-group">
      {#each QUALITY_OPTIONS as opt}
        <label class="pill" class:selected={recording.quality === opt.value}>
          <input
            type="radio"
            name="quality"
            value={opt.value}
            checked={recording.quality === opt.value}
            onchange={() => updateRecording("quality", opt.value)}
          />
          {opt.label}
        </label>
      {/each}
    </div>
  </div>

  <div class="info-box">
    <strong>Requires FFmpeg</strong>
    <span>Install via <code>brew install ffmpeg</code></span>
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

  .format-option:hover { border-color: #bbb; background: #fafafa; }
  .format-option.selected { border-color: #4a9eff; background: #f0f6ff; }

  .format-option input[type="radio"] {
    margin-top: 2px;
    accent-color: #4a9eff;
  }

  .format-content {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .format-name { font-size: 14px; font-weight: 600; }
  .format-desc { font-size: 11px; color: #888; }

  .option-group {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 14px;
    border: 1px solid #ddd;
    border-radius: 20px;
    font-size: 13px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .pill:hover { border-color: #bbb; background: #fafafa; }
  .pill.selected { border-color: #4a9eff; background: #f0f6ff; color: #2a7edf; }

  .pill input[type="radio"] {
    display: none;
  }

  .info-box {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 14px;
    background: #f8f9fb;
    border: 1px solid #e8e8e8;
    border-radius: 8px;
    font-size: 13px;
    color: #666;
  }

  .info-box code {
    background: #eee;
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 12px;
  }
</style>
