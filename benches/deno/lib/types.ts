/**
 * Shared types for benchmark infrastructure
 */

/** Supported source file languages */
export type Language = 'svelte' | 'typescript' | 'css';

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
export function extractVersion(specifier: string): string {
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

/** Statistics about the loaded corpus */
export interface CorpusStats {
	/** Total number of files */
	totalFiles: number;
	/** Total bytes across all files */
	totalBytes: number;
	/** Breakdown by language */
	byLanguage: Record<Language, { files: number; bytes: number }>;
	/** List of repos included */
	repos: string[];
}

/** Implementation names for benchmarking */
export type ImplementationName = 'canonical' | 'native' | 'wasm' | 'oxc' | 'biome';

/** Common interface for parser/formatter implementations */
export interface TsvImplementation {
	name: ImplementationName;

	/** Initialize the implementation (load WASM, open FFI library, etc.) */
	init(): Promise<void>;

	/** Check if parsing is supported for this language */
	supportsParseLanguage(language: Language): boolean;

	/** Check if formatting is supported for this language */
	supportsFormatLanguage(language: Language): boolean;

	/** Parse source and return AST (as object or JSON string) */
	parse(source: string, language: Language): unknown;

	/** Parse source without JSON serialization (native/wasm only, for measuring pure parse speed) */
	parseInternal?(source: string, language: Language): void;

	/** Format source synchronously (native, wasm) */
	format?(source: string, language: Language): string;

	/** Format source asynchronously (canonical/prettier) */
	formatAsync?(source: string, language: Language): Promise<string>;

	/** Clean up resources */
	dispose(): void;
}
