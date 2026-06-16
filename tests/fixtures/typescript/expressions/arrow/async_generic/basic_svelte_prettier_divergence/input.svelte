<script lang="ts">
	//
	// Basic Async Generic Arrow Functions (Svelte context)
	// Single unconstrained type params stay bare here; prettier forces `<T,>` —
	// see README and single_type_param_prettier_divergence.
	//

	// Basic (single unconstrained — tsv bare, prettier `<T,>`)
	const basic = async <T>() => {};

	// With constraint (no trailing comma needed)
	const withConstraint = async <T extends object>() => {};

	// With default (default-only, no constraint)
	const withDefault = async <T = string>() => {};

	// Constraint + default on same param
	const withBoth = async <T extends string = 'default'>() => {};

	// Multiple type parameters
	const multiple = async <T, U>() => {};

	// Multiple with constraints and defaults
	const multipleComplex = async <T extends object, U = T>() => {};

	//
	// Complex Constraints
	//

	// Object type constraint
	const objectConstraint = async <T extends {foo: string}>() => {};

	// Function type constraint
	const fnConstraint = async <T extends () => Promise<void>>() => {};

	// Union type constraint
	const unionConstraint = async <T extends string | number>() => {};

	// Intersection type constraint
	const intersectionConstraint = async <T extends A & B>() => {};

	// Nested generics in constraint
	const nestedGenerics = async <T extends Array<U>, U>() => {};

	// Template literal type constraint
	const templateLiteral = async <T extends `prefix_${string}`>() => {};

	// keyof constraint
	const keyofConstraint = async <T extends object, U extends keyof T>() => {};

	// Conditional type in default
	const conditionalDefault = async <T, U extends T = T extends string ? T : never>() => {};

	//
	// With Parameters and Return Types
	//

	// With parameter
	const withParam = async <T>(x: T): Promise<T> => x;

	// With optional parameter
	const withOptional = async <T>(x?: T): Promise<T | undefined> => x;

	// With rest parameter
	const withRest = async <T>(...args: T[]): Promise<T[]> => args;

	// Array return type
	const arrayReturn = async <T>(): Promise<T[]> => [];

	// Tuple return type
	const tupleReturn = async <T, U>(x: T, y: U): Promise<[T, U]> => [x, y];

	// Readonly array return
	const readonlyReturn = async <T>(): Promise<readonly T[]> => [];

	// Nested generic return
	const nestedReturn = async <T>(): Promise<Awaited<T>> => x;

	//
	// Special Patterns
	//

	// Typed variable with async generic arrow
	const typed: <T>() => Promise<T> = async <T>() => ({}) as T;

	// Async IIFE with type parameter
	(async <T>() => {})();

	// Object body with type assertion
	const objectBody = async <T>(): Promise<T> => ({}) as T;

	// Async instantiation expression
	const instantiation = (async <T>() => {})<string>;
</script>
