<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import RegionSelector from "./RegionSelector.svelte";

  let screenshotBase64 = $state("");
  let loading = $state(true);
  let error = $state("");
  let saved = $state(false);
  let savedMessage = $state("");

  const appWindow = getCurrentWebviewWindow();

  async function captureScreen() {
    try {
      screenshotBase64 = await invoke<string>("get_pre_captured_screen");
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

  function handleSaved(path: string) {
    saved = true;
    savedMessage = `Saved to: ${path}`;
    setTimeout(() => appWindow.close(), 800);
  }

  function handleCopied() {
    saved = true;
    savedMessage = "Copied to clipboard!";
    setTimeout(() => appWindow.close(), 800);
  }

  captureScreen();
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay">
  {#if loading}
    <div class="status-msg loading">Capturing screen...</div>
  {:else if error}
    <div class="status-msg error">{error}</div>
  {:else if saved}
    <div class="status-msg success">{savedMessage}</div>
  {:else}
    <img
      src="data:image/png;base64,{screenshotBase64}"
      alt="Screen capture"
      class="screenshot"
      draggable="false"
    />
    <RegionSelector
      {screenshotBase64}
      onSave={handleSaved}
      onCopy={handleCopied}
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

  .status-msg {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    color: white;
    font-size: 24px;
    font-family: system-ui, sans-serif;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }

  .error { color: #ff4444; }
  .success { color: #44ff44; }
</style>
