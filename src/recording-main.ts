import { mount } from "svelte";
import RecordingApp from "./lib/recording/RecordingApp.svelte";

const app = mount(RecordingApp, {
  target: document.getElementById("app")!,
});

export default app;
