export type ActivationBehavior = "pickOnly" | "pickThenEditor" | "editorFirst";

export type ColorFormat = {
  name: string;
  template: string;
  visible: boolean;
  builtIn: boolean;
};

export type ColorPickerSettings = {
  enabled: boolean;
  hotkey: string;
  activationBehavior: ActivationBehavior;
  defaultFormat: string;
  showColorName: boolean;
  formats: ColorFormat[];
};

export type FormatToken = {
  token: string;
  description: string;
};

export type ColorPickerState = {
  settings: ColorPickerSettings;
  /** `#rrggbb`, newest first. */
  history: string[];
  tokens: FormatToken[];
};

export type FormattedColor = {
  name: string;
  value: string;
  visible: boolean;
};

export type ColorDescription = {
  hex: string;
  name: string;
  formats: FormattedColor[];
};
