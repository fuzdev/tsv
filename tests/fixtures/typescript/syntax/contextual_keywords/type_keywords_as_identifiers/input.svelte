<script lang="ts">
	// All type keywords can be identifiers in value positions

	// number
	{
		const number = 42;
	}
	function fn1(number: number) {
		return number;
	}

	// string
	{
		const string = 'a';
	}
	function fn2(string: string) {
		return string;
	}

	// boolean
	{
		const boolean = true;
	}
	function fn3(boolean: boolean) {
		return boolean;
	}

	// any
	{
		const any = 'x';
	}
	function fn4(any: any) {
		return any;
	}

	// never
	{
		const never = 'a';
	}
	function fn5(never: never) {
		return never;
	}

	// unknown
	{
		const unknown = 'a';
	}
	function fn6(unknown: unknown) {
		return unknown;
	}

	// object
	{
		const object = {};
	}
	function fn7(object: object) {
		return object;
	}

	// bigint
	{
		const bigint = 1n;
	}
	function fn8(bigint: bigint) {
		return bigint;
	}

	// Destructuring with type keywords as property names
	{
		const { number, string, boolean } = obj;
	}

	// Object with type keywords as keys
	const obj2 = {
		number: 1,
		string: 'a',
		boolean: true,
		any: 'x',
		never: 'y',
		unknown: 'z',
		object: {},
		bigint: 1n
	};

	// Assignment
	{
		let number;
		number = 42;
	}
	{
		let string;
		string = 'a';
	}

	// Ambient declaration name (the concrete `function string() {}` form above
	// already parses — `declare` takes the same names)
	declare function string(): void;
	declare function never(): void;
	declare function from<T>(): T[];

	// Type-parameter name
	function fn9<string>(x: string): string {
		return x;
	}
	class A<never> {}
	type T<unknown> = unknown;
	interface I<object> {}

	// Rest-parameter name, in every parameter list
	function fn10(...string: any[]) {}
	const fn11 = (...never: any[]) => never;
	class B {
		m(...unknown: any[]) {}
	}
	const obj3 = { m(...object: any[]) {} };
	declare var a: { m(...bigint: any[]): void };

	// A rest parameter in a FUNCTION TYPE already takes these names
	type F = (...string: any[]) => void;
</script>
