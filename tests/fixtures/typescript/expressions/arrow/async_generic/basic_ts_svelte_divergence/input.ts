// Async generic arrow functions in pure TypeScript (.ts path)
// Single type params stay bare in both contexts for tsv; only prettier forces `<T,>` in Svelte
// See long_svelte_divergence/ (Svelte context) and stacked_svelte_prettier_divergence/ (stacked case)

// Single type param — bare `<T>` (prettier keeps it bare on the .ts path too)
const basic = async <T>(x: T): Promise<T> => x;

// Constrained — no trailing comma either way, but confirms standalone acorn-typescript path
const constrained = async <T extends object>(x: T) => x;

// Multiple type params with function params
const multiple = async <T, U>(x: T, y: U): Promise<[T, U]> => [x, y];

// IIFE — special expression context
(async <T>() => {})();

// Instantiation expression
const instantiation = (async <T>() => {})<string>;
