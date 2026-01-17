// Async generic arrow functions in pure TypeScript
// No trailing comma needed for single type params (no Svelte template syntax disambiguation)
// See basic/input.svelte for Svelte version (trailing comma required)

// Basic - single type param, no constraint
const basic = async <T>() => {};

// With default type
const withDefault = async <T = string>() => {};

// With parameter
const withParam = async <T>(x: T): Promise<T> => x;

// With optional parameter
const withOptional = async <T>(x?: T): Promise<T | undefined> => x;

// With rest parameter
const withRest = async <T>(...args: T[]): Promise<T[]> => args;

// Array return type
const arrayReturn = async <T>(): Promise<T[]> => [];

// Readonly array return
const readonlyReturn = async <T>(): Promise<readonly T[]> => [];

// Nested generic return
const nestedReturn = async <T>(): Promise<Awaited<T>> => ({}) as Awaited<T>;

// Typed variable with async generic arrow
const typed: <T>() => Promise<T> = async <T>() => ({}) as T;

// Async IIFE with type parameter
(async <T>() => {})();

// Object body with type assertion
const objectBody = async <T>(): Promise<T> => ({}) as T;

// Async instantiation expression
const instantiation = (async <T>() => {})<string>;

// With constraint - these never need trailing comma anyway
const withConstraint = async <T extends object>() => {};
const withBoth = async <T extends string = 'default'>() => {};

// Multiple type params - no trailing comma needed (already disambiguated)
const multiple = async <T, U>() => {};
const multipleComplex = async <T extends object, U = T>() => {};
