import { mount } from "svelte";
import "./app.css";
import App from "./App.svelte";
import { setTheme } from "./lib/util/theme";

// Follow the OS theme until settings are loaded (avoids a flash).
setTheme("system");

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
