// Deno Sidecar for tsv_debug
// Long-running process for JS tools. Communicates via JSON-lines over stdio.

// Version constants - keep in sync with imports below
const VERSIONS = {
  "prettier": "3.7.4",
  "prettier-plugin-svelte": "3.4.0",
  "svelte": "5.45.8",
  "acorn": "8.14.0",
  "@sveltejs/acorn-typescript": "1.0.5",
} as const;

import * as prettier from "npm:prettier@^3.7.4";
import prettierPluginSvelte from "npm:prettier-plugin-svelte@^3.4.0";
import { parse as svelteParse } from "npm:svelte@^5.45.8/compiler";
import * as acorn from "npm:acorn@^8.14.0";
import { tsPlugin } from "npm:@sveltejs/acorn-typescript@^1.0.5";
import { TextLineStream } from "jsr:@std/streams@1/text-line-stream";

// Create TypeScript-enabled parser
const ParserWithTS = acorn.Parser.extend(tsPlugin());

interface Request {
  id: number;
  tool: string;
  content: string;
  options?: Record<string, unknown>;
}

// JSON replacer that converts BigInt to string (BigInt can't be serialized natively)
function jsonReplacer(_key: string, value: unknown): unknown {
  return typeof value === "bigint" ? value.toString() : value;
}

interface Response {
  id: number;
  ok: boolean;
  output?: unknown;
  error?: string;
  duration_ms: number;
}

async function dispatch(
  tool: string,
  content: string,
  options?: Record<string, unknown>,
): Promise<unknown> {
  switch (tool) {
    case "__version_info": {
      return {
        runtime: Deno.version.deno,
        typescript: Deno.version.typescript,
        dependencies: VERSIONS,
      };
    }

    case "prettier": {
      // Provide default filepath based on parser to help prettier make correct decisions
      // (e.g., typescript parser without filepath hint might add unnecessary JSX disambiguation)
      const filepath = options?.filepath ?? (
        options?.parser === "typescript" ? "file.ts" :
        options?.parser === "svelte" ? "file.svelte" :
        options?.parser === "css" ? "file.css" :
        undefined
      );
      return await prettier.format(content, {
        plugins: [prettierPluginSvelte],
        useTabs: true,
        printWidth: 100,
        singleQuote: true,
        bracketSpacing: false,
        parser: options?.parser,
        filepath,
      });
    }

    case "svelte-parse": {
      const ast = svelteParse(content, { modern: true });
      return JSON.stringify(ast, jsonReplacer, "\t");
    }

    case "acorn-ts-parse": {
      const ast = ParserWithTS.parse(content, {
        sourceType: "module",
        ecmaVersion: 2025,
        locations: true,
      });
      return JSON.stringify(ast, jsonReplacer, "\t");
    }

    default:
      throw new Error(`Unknown tool: ${tool}`);
  }
}

// Main loop: read JSON-lines from stdin, process, write responses to stdout
const lines = Deno.stdin.readable
  .pipeThrough(new TextDecoderStream())
  .pipeThrough(new TextLineStream());

for await (const line of lines) {
  const req: Request = JSON.parse(line);
  const start = performance.now();

  let response: Response;
  try {
    const output = await dispatch(req.tool, req.content, req.options);
    response = {
      id: req.id,
      ok: true,
      output,
      duration_ms: Math.round(performance.now() - start),
    };
  } catch (err) {
    response = {
      id: req.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
      duration_ms: Math.round(performance.now() - start),
    };
  }

  console.log(JSON.stringify(response));
}
