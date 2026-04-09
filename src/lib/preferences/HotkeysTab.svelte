<script lang="ts">
interface Props {
  bindings: Record<string, string>;
  onChangeBinding: (action: string, newBinding: string) => void;
}

let { bindings, onChangeBinding }: Props = $props();

let recording = $state<string | null>(null);

interface HotkeyEntry {
  id: string;
  label: string;
}

const GLOBAL_ACTIONS: HotkeyEntry[] = [
  { id: "screenshot", label: "Screenshot" },
  { id: "capture_last_region", label: "Capture Last Region" },
  { id: "record_screen", label: "Record Screen" },
  { id: "open_history", label: "Open History" },
  { id: "open_preferences", label: "Open Preferences" },
];

const TOOL_ACTIONS: HotkeyEntry[] = [
  { id: "tool_arrow", label: "Arrow" },
  { id: "tool_rect", label: "Rectangle" },
  { id: "tool_circle", label: "Circle" },
  { id: "tool_line", label: "Line" },
  { id: "tool_dottedline", label: "Dotted Line" },
  { id: "tool_freehand", label: "Freehand" },
  { id: "tool_text", label: "Text" },
  { id: "tool_highlight", label: "Highlight" },
  { id: "tool_blur", label: "Blur" },
  { id: "tool_steps", label: "Steps" },
  { id: "tool_colorpicker", label: "Color Picker" },
  { id: "tool_measure", label: "Measurement" },
];

const SIZE_ACTIONS: HotkeyEntry[] = [
  { id: "size_small", label: "Small" },
  { id: "size_medium", label: "Medium" },
  { id: "size_large", label: "Large" },
];

const OVERLAY_ACTIONS: HotkeyEntry[] = [
  { id: "action_save", label: "Save" },
  { id: "action_copy", label: "Copy to Clipboard" },
  { id: "action_undo", label: "Undo" },
  { id: "action_redo", label: "Redo" },
  { id: "action_cancel", label: "Cancel" },
];

let conflictMsg = $state("");

function startRecording(action: string) {
  recording = action;
  conflictMsg = "";
}
function cancelRecording() {
  recording = null;
  conflictMsg = "";
}

/** Find which action already uses a given binding string. */
function findConflict(newBinding: string, excludeAction: string): string | null {
  const allActions = [...GLOBAL_ACTIONS, ...TOOL_ACTIONS, ...SIZE_ACTIONS, ...OVERLAY_ACTIONS];
  for (const a of allActions) {
    if (a.id !== excludeAction && bindings[a.id] === newBinding) {
      return a.label;
    }
  }
  return null;
}

function handleKeydown(e: KeyboardEvent) {
  if (!recording) return;
  e.preventDefault();
  e.stopPropagation();
  if (e.key === "Escape") {
    recording = null;
    conflictMsg = "";
    return;
  }
  if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) return;

  const parts: string[] = [];
  if (e.ctrlKey || e.metaKey) parts.push("CmdOrCtrl");
  if (e.shiftKey) parts.push("Shift");
  if (e.altKey) parts.push("Alt");

  let key = e.key.length === 1 ? e.key.toUpperCase() : e.code;
  if (key.startsWith("Key")) key = key.slice(3);
  if (key.startsWith("Digit")) key = key.slice(5);
  parts.push(key);

  const newBinding = parts.join("+");
  const conflict = findConflict(newBinding, recording);
  if (conflict) {
    conflictMsg = `"${fmt(newBinding)}" is already used by "${conflict}"`;
    return;
  }

  onChangeBinding(recording, newBinding);
  recording = null;
  conflictMsg = "";
}

function resetDefaults() {
  const defaults: Record<string, string> = {
    screenshot: "CmdOrCtrl+Shift+S",
    capture_last_region: "CmdOrCtrl+Shift+L",
    record_screen: "CmdOrCtrl+Shift+R",
    open_history: "CmdOrCtrl+Shift+H",
    open_preferences: "CmdOrCtrl+Comma",
    tool_arrow: "A",
    tool_rect: "R",
    tool_circle: "C",
    tool_line: "L",
    tool_dottedline: "D",
    tool_freehand: "F",
    tool_text: "T",
    tool_highlight: "H",
    tool_blur: "B",
    tool_steps: "N",
    tool_colorpicker: "I",
    tool_measure: "M",
    size_small: "1",
    size_medium: "2",
    size_large: "3",
    action_save: "CmdOrCtrl+S",
    action_copy: "CmdOrCtrl+C",
    action_undo: "CmdOrCtrl+Z",
    action_redo: "CmdOrCtrl+Shift+Z",
    action_cancel: "Escape",
  };
  for (const [k, v] of Object.entries(defaults)) onChangeBinding(k, v);
}

function fmt(s: string): string {
  return s.replace("CmdOrCtrl", "⌘").replace("Shift", "⇧").replace("Alt", "⌥").replace(/\+/g, " ");
}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="tab-content">
  {#each [
    { title: "Global Shortcuts", items: GLOBAL_ACTIONS, col: "Action" },
    { title: "Annotation Tools", items: TOOL_ACTIONS, col: "Tool" },
    { title: "Stroke Size", items: SIZE_ACTIONS, col: "Size" },
    { title: "Overlay Actions", items: OVERLAY_ACTIONS, col: "Action" },
  ] as section}
    <section>
      <h3>{section.title}</h3>
      <table>
        <thead><tr><th>{section.col}</th><th>Shortcut</th><th></th></tr></thead>
        <tbody>
          {#each section.items as action}
            <tr class:active={recording === action.id}>
              <td class="name">{action.label}</td>
              <td class="binding">
                {#if recording === action.id}
                  <span class="rec">Press keys...</span>
                {:else}
                  <kbd>{fmt(bindings[action.id] ?? "")}</kbd>
                {/if}
              </td>
              <td class="btn-cell">
                {#if recording === action.id}
                  <button class="cancel" onclick={cancelRecording}>Cancel</button>
                {:else}
                  <button class="change" onclick={() => startRecording(action.id)}>Change</button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  {/each}

  {#if conflictMsg}
    <div class="conflict-warning">{conflictMsg}</div>
  {/if}

  <button class="reset" onclick={resetDefaults}>Reset All to Defaults</button>
</div>

<style>
  .tab-content { display: flex; flex-direction: column; gap: 22px; }
  section { display: flex; flex-direction: column; gap: 6px; }
  h3 { font-size: 12px; font-weight: 600; color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.5px; }

  table { width: 100%; border-collapse: collapse; }
  thead th { text-align: left; padding: 5px 8px; font-size: 11px; font-weight: 500; color: var(--text-faint); border-bottom: 1px solid var(--border); }
  tbody tr { transition: background 0.1s; }
  tbody tr:hover { background: var(--bg-hover); }
  tbody tr.active { background: var(--bg-active); }
  td { padding: 5px 8px; border-bottom: 1px solid var(--border-light); font-size: 13px; vertical-align: middle; }

  .name { font-weight: 450; color: var(--text); }
  .binding { width: 130px; }
  .binding kbd {
    background: var(--bg-kbd);
    border: 1px solid var(--border-kbd);
    padding: 1px 7px;
    border-radius: 4px;
    font-size: 12px;
    font-family: system-ui, sans-serif;
    letter-spacing: 0.5px;
    color: var(--text);
  }
  .btn-cell { width: 65px; text-align: right; }

  .rec { color: var(--accent); font-size: 12px; font-style: italic; animation: blink 1s ease-in-out infinite; }
  @keyframes blink { 0%, 100% { opacity: 1; } 50% { opacity: 0.4; } }

  .change, .cancel {
    padding: 2px 9px;
    border: 1px solid var(--border-input);
    border-radius: 4px;
    background: var(--bg);
    cursor: pointer;
    font-size: 11px;
    color: var(--text-secondary);
  }
  .change:hover { background: var(--bg-hover); }
  .cancel { border-color: var(--danger-border); color: var(--danger); }
  .cancel:hover { background: var(--danger-bg); }

  .reset {
    align-self: flex-start;
    padding: 5px 14px;
    border: 1px solid var(--border-input);
    border-radius: 5px;
    background: var(--bg);
    cursor: pointer;
    font-size: 12px;
    color: var(--text-secondary);
  }
  .reset:hover { background: var(--bg-hover); }

  .conflict-warning {
    background: rgba(255, 80, 0, 0.1);
    border: 1px solid rgba(255, 80, 0, 0.3);
    color: #ff6633;
    padding: 6px 12px;
    border-radius: 6px;
    font-size: 12px;
  }
</style>
