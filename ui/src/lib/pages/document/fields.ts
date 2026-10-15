import type { IconName } from "../../components/Icon.svelte";

export interface FieldDef {
  label: string;
  value: string | null | undefined;
  wide?: boolean;
  mono?: boolean;
  suffix?: string;
  /** hide the field entirely when empty */
  optional?: boolean;
}

export interface SectionDef {
  title: string;
  icon: IconName;
  fields: FieldDef[];
  note?: string;
}

export type Tone = "accent" | "success" | "warning" | "danger" | "neutral";

export interface BadgeDef {
  label: string;
  tone: Tone;
  icon?: IconName;
}

export function visible(fields: FieldDef[]): FieldDef[] {
  return fields.filter((f) => !f.optional || (f.value && f.value.trim()));
}
