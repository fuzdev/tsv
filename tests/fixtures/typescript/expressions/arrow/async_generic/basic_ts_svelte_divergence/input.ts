// Async generic arrow functions in pure TypeScript (.ts path)
// No trailing comma needed for single type params (unlike .svelte)
// See basic_svelte_divergence/input.svelte for full coverage

// Single type param — no trailing comma (the .ts-specific difference)
const basic = async <T>(x: T): Promise<T> => x;

// Constrained — no trailing comma either way, but confirms standalone acorn-typescript path
const constrained = async <T extends object>(x: T) => x;

// Multiple type params with function params
const multiple = async <T, U>(x: T, y: U): Promise<[T, U]> => [x, y];

// IIFE — special expression context
(async <T>() => {})();

// Instantiation expression
const instantiation = (async <T>() => {})<string>;
