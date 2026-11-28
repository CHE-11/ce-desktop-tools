import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { applyAppearance, readAppearance } from "./lib/appearance";
import { installDevConsole } from "./lib/dev-feedback";
import { applyZoom, readZoom } from "./lib/zoom";
import { LauncherWindow } from "./components/Tools/Launcher/LauncherWindow";
import { PantherbarBar } from "./components/Tools/Pantherbar/PantherbarBar";
import { HomePage } from "./Home.page";
import "./index.css";
import { applyDensity, readDensity } from "./lib/density";
import { ColorEditorWindow } from "./components/Tools/ColorPicker/ColorEditorWindow";
import { RulerToolbar } from "./components/Tools/ScreenRuler/RulerToolbar";
import { PresentPalette } from "./components/Tools/Present/PresentPalette";
import { PresentStageWindow } from "./components/Tools/Present/PresentStageWindow";
import { RecordCamera } from "./components/Tools/Record/RecordCamera";
import { RecordControls } from "./components/Tools/Record/RecordControls";
import { RecordDraw } from "./components/Tools/Record/RecordDraw";

if (import.meta.env.DEV) {
  installDevConsole();
}

const appearance = readAppearance();
applyAppearance(appearance.theme, appearance.colorMode);
applyDensity(readDensity());
applyZoom(readZoom());
installNoInputSuggestions();

const pantherbarWindow = window.__PANTHERBAR__ === true;
const launcherWindow = window.__LAUNCHER__ === true;
const screenRulerWindow = window.__SCREEN_RULER__ === true;
const colorPickerWindow = window.__COLOR_PICKER__ === true;
const recordControlsWindow = window.__RECORD_CONTROLS__ === true;
const recordCameraWindow = window.__RECORD_CAMERA__ === true;
const recordDrawWindow = window.__RECORD_DRAW__ === true;
const presentStage = typeof window.__PRESENT_STAGE__ === "number" ? window.__PRESENT_STAGE__ : null;
const presentPaletteWindow = window.__PRESENT_PALETTE__ === true;
if (recordControlsWindow || recordCameraWindow || recordDrawWindow) {
  document.documentElement.dataset.window = "record";
} else if (presentStage !== null || presentPaletteWindow) {
  document.documentElement.dataset.window = "present";
} else if (pantherbarWindow) {
  document.documentElement.dataset.window = "pantherbar";
} else if (launcherWindow) {
  document.documentElement.dataset.window = "launcher";
} else if (screenRulerWindow) {
  document.documentElement.dataset.window = "screen-ruler";
} else if (colorPickerWindow) {
  document.documentElement.dataset.window = "color-picker";
}

const root = document.getElementById("root");
if (!root) {
  throw new Error("Root element is missing");
}

function WindowContent() {
  if (recordControlsWindow) {
    return <RecordControls />;
  }
  if (recordCameraWindow) {
    return <RecordCamera />;
  }
  if (recordDrawWindow) {
    return <RecordDraw />;
  }
  if (presentStage !== null) {
    return <PresentStageWindow monitor={presentStage} />;
  }
  if (presentPaletteWindow) {
    return <PresentPalette />;
  }
  if (pantherbarWindow) {
    return <PantherbarBar />;
  }
  if (launcherWindow) {
    return <LauncherWindow />;
  }
  if (screenRulerWindow) {
    return <RulerToolbar />;
  }
  if (colorPickerWindow) {
    return <ColorEditorWindow />;
  }
  return <HomePage />;
}

createRoot(root).render(
  <StrictMode>
    <WindowContent />
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
