import { mount } from "svelte";
import "@fontsource-variable/golos-text";
import "./app.css";
import Translator from "./Translator.svelte";

mount(Translator, { target: document.getElementById("translator")! });
