/** Target code generation from checked project IR. */

import { generateJavaScript } from "@neo-astra/codegen-js";
import type { ProjectCheckResult, ProjectSource, StructuredDiagnostic } from "@neo-astra/playground/protocol";
import type { EffectiveSettings } from "./settings.js";

export interface CompiledFile {
  readonly key: string;
  readonly path: string;
  readonly code: string;
}

export interface CompiledProject {
  readonly target: string;
  readonly files: readonly CompiledFile[];
  readonly diagnostics: readonly StructuredDiagnostic[];
}

function moduleSpecifier(key: string): string {
  return `@playground/${encodeURIComponent(key)}`;
}

export function compileProject(
  checked: ProjectCheckResult,
  sources: readonly ProjectSource[],
  settings: EffectiveSettings,
): CompiledProject {
  const backend = settings.target === "ts" ? "ts" : "js";
  const extension = backend === "ts" ? "ts" : "js";
  const specifiers: Record<string, string> = Object.fromEntries(
    checked.modules.map((module) => [module.key, moduleSpecifier(module.key)]),
  );
  const sourceText = new Map(sources.map((source) => [source.key, source.text]));
  const files: CompiledFile[] = [];
  const diagnostics: StructuredDiagnostic[] = [];

  for (const module of checked.modules) {
    if (!module.ok || module.ir === undefined) continue;
    const text = sourceText.get(module.key);
    const generated = generateJavaScript({
      moduleName: module.key,
      ir: module.ir,
      ...(text === undefined ? {} : { sourceText: text }),
      backend,
      moduleSpecifiers: specifiers,
    });
    files.push({
      key: module.key,
      path: module.path.replace(/\.ns$/i, `.${extension}`),
      code: generated.code,
    });
    for (const item of generated.diagnostics) {
      diagnostics.push({
        code: item.code,
        severity: "error",
        message: item.message,
        span: item.span === undefined ? null : { start: item.span.start, end: item.span.end },
        hint: null,
      });
    }
  }

  return { target: settings.target, files, diagnostics };
}
