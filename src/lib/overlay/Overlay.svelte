<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import RegionSelector from "./RegionSelector.svelte";

  let screenshotBase64 = $state("");
  let loading = $state(true);
  let error = $state("");
  let regionSelected = $state(false);

  const appWindow = getCurrentWebviewWindow();

  async function captureScreen() {
    try {
      screenshotBase64 = await invoke<string>("capture_screen", { display: 0 });
      loading = false;
    } catch (e) {
      error = String(e);
      loading = false;
    }
  }

  function handleCancel() {
    appWindow.close();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      handleCancel();
    }
  }

  async function handleRegionSaved(path: string) {
    regionSelected = true;
    setTimeout(() => appWindow.close(), 500);
  }

  captureScreen();
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay">
  {#if loading}
    <div class="loading">Capturing screen...</div>
  {:else if error}
    <div class="error">{error}</div>
  {:else if regionSelected}
    <div class="success">Screenshot saved!</div>
  {:else}
    <img
      src="data:image/png;base64,{screenshotBase64}"
      alt="Screen capture"
      class="screenshot"
      draggable="false"
    />
    <RegionSelector
      onSave={handleRegionSaved}
      onCancel={handleCancel}
    />
  {/if}
</div>

<style>
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
  }

  .screenshot {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    pointer-events: none;
  }

  .loading, .error, .success {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    color: white;
    font-size: 24px;
    font-family: system-ui, sans-serif;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }

  .error {
    color: #ff4444;
  }

  .success {
    color: #44ff44;
  }
</style>
