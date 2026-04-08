<script lang="ts">
import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import RegionSelector from "./RegionSelector.svelte";

const appWindow = getCurrentWebviewWindow();

const urlParams = new URLSearchParams(window.location.search);
const overlayPurpose: "screenshot" | "record" =
  urlParams.get("mode") === "record" ? "record" : "screenshot";
const displayIndex = Number(urlParams.get("display") ?? "0");

// Listen for signals from sibling overlays
listen("overlay-done", () => {
  appWindow.close();
});
listen("overlay-cancel", () => {
  appWindow.close();
});

function handleCancel() {
  emit("overlay-cancel");
  appWindow.close();
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    handleCancel();
  }
}

function handleSaved(path: string) {
  invoke("show_toast", { message: `Saved to: ${path}` });
  emit("overlay-done");
  appWindow.close();
}

function handleCopied() {
  invoke("show_toast", { message: "Copied to clipboard!" });
  emit("overlay-done");
  appWindow.close();
}

function handleRecordingStarted(path: string) {
  emit("overlay-done");
  appWindow.close();
}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="overlay">
  <RegionSelector
    purpose={overlayPurpose}
    display={displayIndex}
    onSave={handleSaved}
    onCopy={handleCopied}
    onRecordingStarted={handleRecordingStarted}
    onCancel={handleCancel}
  />
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
</style>
