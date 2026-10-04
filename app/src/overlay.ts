import { mount } from "svelte";
import "@fontsource-variable/golos-text";
import Overlay from "./Overlay.svelte";

mount(Overlay, { target: document.getElementById("overlay")! });
