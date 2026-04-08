import { mount } from "svelte";
import HistoryApp from "./lib/history/HistoryApp.svelte";

const app = mount(HistoryApp, {
  target: document.getElementById("app")!,
});

export default app;
