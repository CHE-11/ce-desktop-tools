export type CalcRef = {
  id: number;
  base: number;
  dimension: string | null;
  unit: string | null;
};

export type CalcResult = {
  ok: boolean;
  title: string;
  value: string;
  unit: string | null;
  dimension: string | null;
  base: number | null;
  message: string;
};
