export type UnitsKind = "conversion" | "math" | "message";

export type UnitRow = {
  symbol: string;
  label: string;
  value: string;
};

export type UnitsAnswer = {
  kind: UnitsKind;
  title: string;
  value: string;
  dimension: string | null;
  rows: UnitRow[];
  from: string | null;
  to: string | null;
};

export type CatalogDimension = {
  id: string;
  label: string;
};

export type CatalogUnit = {
  symbol: string;
  label: string;
  dimension: string;
};

export type UnitsCatalog = {
  dimensions: CatalogDimension[];
  units: CatalogUnit[];
};
