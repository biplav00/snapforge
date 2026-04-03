<script lang="ts">
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import RegionSelector from "./RegionSelector.svelte";

  let saved = $state(false);
  let savedMessage = $state("");

  const appWindow = getCurrentWebviewWindow();

  // Determine overlay purpose from URL query param
  const urlParams = new URLSearchParams(window.location.search);
  const overlayPurpose: "screenshot" | "record" = urlParams.get("mode") === "record" ? "record" : "screenshot";

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
    // Don't close — stay in annotation mode (handled by RegionSelector toast)
  }

  function handleRecordingStarted(path: string) {
    appWindow.close();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay">
  {#if saved}
    <div class="status-msg success">{savedMessage}</div>
  {:else}
    <RegionSelector
      purpose={overlayPurpose}
      onSave={handleSaved}
      onCopy={handleCopied}
      onRecordingStarted={handleRecordingStarted}
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

  .success { color: #44ff44; }
</style>
