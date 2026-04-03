import Preferences from "./lib/preferences/Preferences.svelte";
import { mount } from "svelte";

const app = mount(Preferences, {
  target: document.getElementById("app")!,
});

export default app;
