/**
 * Shared types for benchmark infrastructure
 */

/** Logger function type */
export type Logger = (...args: unknown[]) => void;

/** Supported source file languages */
export type Language = 'svelte' | 'typescript' | 'css';

/** All supported languages as an array */
export const LANGUAGES: Language[] = ['svelte', 'typescript', 'css'];

/** File extensions for each language */
export const LANGUAGE_EXTENSIONS: Record<Language, string> = {
	svelte: '.svelte',
	typescript: '.ts',
	css: '.css',
};

/** Prettier parser names for each language */
export const LANGUAGE_PRETTIER_PARSERS: Record<Language, string> = {
	svelte: 'svelte',
	typescript: 'typescript',
	css: 'css',
};

/** Extract version from npm specifier (e.g., "npm:prettier@3.7.4" -> "3.7.4") */
export function extract_version(specifier: string): string {
	const match = specifier.match(/@(\d+\.\d+\.\d+)/);
	return match ? match[1] : 'unknown';
}

/** A source file loaded into memory for benchmarking */
export interface SourceFile {
	/** Absolute path to the file */
	path: string;
	/** File content (pre-loaded) */
	content: string;
	/** Detected language based on extension */
	language: Language;
	/** Size in bytes */
	bytes: number;
}

/** Implementation names for benchmarking */
export type ImplementationName =
	| 'canonical'
	| 'native'
	| 'wasm'
	| 'oxc'
	| 'oxc-wasm'
	| 'biome-wasm';

/** Common interface for parser/formatter implementations */
export interface TsvImplementation {
	name: ImplementationName;

	/** Initialize the implementation (load WASM, open FFI library, etc.) */
	init(): Promise<void>;

	/** Check if parsing is supported for this language */
	supports_parse_language(language: Language): boolean;

	/** Check if formatting is supported for this language */
	supports_format_language(language: Language): boolean;

	/** Parse source and return AST (as object or JSON string) */
	parse(source: string, language: Language): unknown;

	/** Parse source without JSON serialization (native/wasm only, for measuring pure parse speed) */
	parse_internal?(source: string, language: Language): void;

	/** Format source synchronously (native, wasm) */
	format?(source: string, language: Language): string;

	/** Format source asynchronously (canonical/prettier) */
	format_async?(source: string, language: Language): Promise<string>;

	/** Clean up resources */
	dispose(): void;
}
