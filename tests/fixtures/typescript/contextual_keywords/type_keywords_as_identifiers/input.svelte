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
		const {number, string, boolean} = obj;
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
		bigint: 1n,
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
</script>
