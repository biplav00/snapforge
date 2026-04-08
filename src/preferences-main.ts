import { mount } from "svelte";
import PreferencesApp from "./lib/preferences/PreferencesApp.svelte";

const app = mount(PreferencesApp, {
  target: document.getElementById("app")!,
});

export default app;
