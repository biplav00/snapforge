<script lang="ts">
  interface Props {
    onSave: (path: string) => void;
    onCancel: () => void;
  }

  let { onSave, onCancel }: Props = $props();
</script>

<div class="region-placeholder">
  <p>Region selector loading... (Press Escape to cancel)</p>
</div>

<style>
  .region-placeholder {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    color: white;
    font-size: 18px;
    font-family: system-ui, sans-serif;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
  }
</style>
