<script lang="ts">
	//
	// Generic Arrow Functions
	//
	// Single-unconstrained and default-only type params (`<T>` / `<T = string>`)
	// live in single_type_param_prettier_divergence — prettier forces `<T,>` there.

	// Multiple params with as const
	const pair = <T, U>(a: T, b: U) => [a, b] as const;

	// Type parameter with constraint only
	const withConstraint = <T extends string>(x: T): T => x;

	// Type parameter with both constraint and default
	const withBoth = <T extends string = 'default'>(x: T): T => x;

	// Multiple type parameters
	const multiple = <T, U>(x: T, y: U): [T, U] => [x, y];

	// Multiple type parameters with various constraints/defaults
	const complexParams = <T, U extends T, V = number>(x: T, y: U, z: V) => [x, y, z];

	// Complex constraint (object type)
	const objectConstraint = <T extends {foo: string}>(x: T): T => x;

	// Complex constraint (function type)
	const fnConstraint = <T extends (x: string) => number>(f: T): T => f;

	// Generic arrow with conditional type in default
	const conditionalDefault = <T, U extends T = T extends string ? T : never>(x: T) => x;

	//
	// Instantiation Expressions
	//

	// Basic instantiation expression (PR #17724)
	void (<_T extends never>() => {})<never>;

	// Simple instantiation
	const simple = withConstraint<string>;

	// Multiple type arguments
	const multipleArgs = multiple<string, number>;

	// Complex type argument
	const complexArg = withConstraint<'literal'>;

	// Nested instantiation (parenthesized arrow)
	const nestedInst = (<T extends string>(x: T): T => x)<string>;

	// Instantiation in conditional
	const conditional = true ? withConstraint<string> : withConstraint<number>;
</script>
