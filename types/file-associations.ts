export type FileAssociation = {
  id: string;
  extension: string;
  programPath: string;
  arguments: string;
  label: string;
  iconPath: string | null;
  previousProgId: string | null;
};

export type FileAssociationStatus = "applied" | "userChoice" | "notApplied";

export type FileAssociationView = FileAssociation & {
  status: FileAssociationStatus;
  command: string;
  userChoiceProgId: string | null;
};

export type FileAssociationsState = {
  settingsFolder: string;
  associations: FileAssociationView[];
  error: string | null;
};

export type ExtensionHandler = {
  extension: string;
  progId: string | null;
  source: "userChoice" | "classes" | "none";
  friendlyName: string | null;
  command: string | null;
};
