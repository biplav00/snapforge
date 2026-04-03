import Indicator from "./lib/recording/Indicator.svelte";
import { mount } from "svelte";

const app = mount(Indicator, {
  target: document.getElementById("app")!,
});

export default app;
