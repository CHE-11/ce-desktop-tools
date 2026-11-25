export type DesktopAppKind = "store" | "desktop";

export type DesktopApp = {
  name: string;
  appId: string;
  onDesktop: boolean;
  publisher: string | null;
  version: string | null;
  kind: DesktopAppKind;
  location: string | null;
  /** PNG data URL, when Windows can resolve an icon. */
  icon: string | null;
};
