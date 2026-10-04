import { mount } from "svelte";
import "@fontsource-variable/golos-text";
import "@fontsource/jetbrains-mono/400.css";
import App from "./App.svelte";
import "./app.css";

mount(App, { target: document.getElementById("app")! });
