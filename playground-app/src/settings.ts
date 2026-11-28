/** Effective compiler settings after UI overrides are applied. */

import type { Edition, IntegerPolicy, Mode } from "@neo-astra/playground/protocol";
import type { ProjectSettings } from "./manifest.js";

export type EditionOverride = Edition | "auto";
export type ModeOverride = Mode | "auto";
export type IntegerPolicyOverride = IntegerPolicy | "auto";

export interface UiOverrides {
  readonly edition: EditionOverride;
  readonly target: string;
  readonly mode: ModeOverride;
  readonly integerPolicy: IntegerPolicyOverride;
}

export interface EffectiveSettings {
  readonly apiVersion: "1.0";
  readonly edition: Edition;
  readonly target: string;
  readonly mode: Mode | undefined;
  readonly integerPolicy: IntegerPolicy | undefined;
}

export const DEFAULT_OVERRIDES: UiOverrides = {
  edition: "auto",
  target: "",
  mode: "auto",
  integerPolicy: "auto",
};

function pick<T extends string>(override: T | "auto", fallback: T | undefined): T | undefined {
  return override === "auto" ? fallback : override;
}

export function resolveSettings(
  fromManifest: ProjectSettings,
  overrides: UiOverrides,
): EffectiveSettings {
  return {
    apiVersion: "1.0",
    edition: pick(overrides.edition, fromManifest.edition) ?? "0.1",
    target: overrides.target === "" ? fromManifest.target : overrides.target,
    mode: pick(overrides.mode, fromManifest.mode),
    integerPolicy: pick(overrides.integerPolicy, fromManifest.integerPolicy),
  };
}

export function withOptionalFields(settings: EffectiveSettings): Record<string, unknown> {
  return {
    apiVersion: settings.apiVersion,
    target: settings.target,
    edition: settings.edition,
    ...(settings.mode === undefined ? {} : { mode: settings.mode }),
    ...(settings.integerPolicy === undefined ? {} : { integerPolicy: settings.integerPolicy }),
  };
}
