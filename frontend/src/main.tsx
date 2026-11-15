import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyAppearance, readAppearance } from "./lib/appearance";
import { installDevConsole } from "./lib/dev-feedback";
import { applyZoom, readZoom } from "./lib/zoom";
import { PantherbarBar } from "./components/Tools/PantherbarBar";
import { HomePage } from "./Home.page";
import "./index.css";
import { applyDensity, readDensity } from "./lib/density";

if (import.meta.env.DEV) {
  installDevConsole();
}

const appearance = readAppearance();
applyAppearance(appearance.theme, appearance.colorMode);
applyDensity(readDensity());
applyZoom(readZoom());
installNoInputSuggestions();

const pantherbarWindow = window.__PANTHERBAR__ === true;
if (pantherbarWindow) {
  document.documentElement.dataset.window = "pantherbar";
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("Root element is missing");
}

createRoot(root).render(
  <StrictMode>
    {pantherbarWindow ? <PantherbarBar /> : <HomePage />}
  </StrictMode>,
);

const SUGGESTION_SKIP = new Set(["button", "checkbox", "color", "file", "hidden", "image", "radio", "range", "reset", "submit"]);

function installNoInputSuggestions() {
  const silence = (node: Node) => {
    if (!(node instanceof Element)) {
      return;
    }
    const fields: Element[] = [];
    if (node.matches("input, textarea, form")) {
      fields.push(node);
    }
    fields.push(...node.querySelectorAll("input, textarea, form"));
    for (const field of fields) {
      if (field instanceof HTMLFormElement) {
        field.autocomplete = "off";
        continue;
      }
      if (!(field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement)) {
        continue;
      }
      if (field instanceof HTMLInputElement && SUGGESTION_SKIP.has(field.type)) {
        continue;
      }
      field.autocomplete = "off";
      field.spellcheck = false;
      field.autocapitalize = "off";
      field.setAttribute("autocorrect", "off");
    }
  };

  silence(document.documentElement);
  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      mutation.addedNodes.forEach(silence);
    }
  });
  observer.observe(document.documentElement, { childList: true, subtree: true });
}
