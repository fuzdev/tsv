/**
 * Shared types for benchmark infrastructure
 */

/** Supported source file languages */
export type Language = 'svelte' | 'typescript' | 'css';

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

/** Implementation being benchmarked */
export type Implementation = 'canonical' | 'native' | 'wasm';

/** Operation being benchmarked */
export type Operation = 'parse' | 'format';

/** Result from a single benchmark iteration (for aggregation) */
export interface BenchmarkResult {
	implementation: Implementation;
	operation: Operation;
	language: Language;
	/** Average time in nanoseconds */
	avgNs: number;
	/** Minimum time in nanoseconds */
	minNs: number;
	/** Maximum time in nanoseconds */
	maxNs: number;
	/** Number of iterations */
	iterations: number;
}

/** Full benchmark report */
export interface BenchmarkReport {
	/** ISO timestamp when benchmark was run */
	timestamp: string;
	/** Corpus statistics */
	corpus: CorpusStats;
	/** Individual benchmark results */
	results: BenchmarkResult[];
	/** Computed summary statistics */
	summary: {
		/** Native parse speedup vs canonical (e.g., 15.3 means 15.3x faster) */
		parseSpeedupNative: Record<Language, number>;
		/** WASM parse speedup vs canonical */
		parseSpeedupWasm: Record<Language, number>;
		/** Native format speedup vs canonical */
		formatSpeedupNative: Record<Language, number>;
		/** WASM format speedup vs canonical */
		formatSpeedupWasm: Record<Language, number>;
	};
}

/** Common interface for parser/formatter implementations */
export interface TsvImplementation {
	name: Implementation;

	/** Initialize the implementation (load WASM, open FFI library, etc.) */
	init(): Promise<void>;

	/** Parse source and return AST (as object or JSON string) */
	parse(source: string, language: Language): unknown;

	/** Format source and return formatted string */
	format(source: string, language: Language): string;

	/** Clean up resources */
	dispose(): void;
}
