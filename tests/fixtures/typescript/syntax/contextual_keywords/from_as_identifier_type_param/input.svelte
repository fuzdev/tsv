<script lang="ts">
	// function type parameter
	type A = (from: number, to: number) => void;

	// generic type argument
	type B = Set<(from: number, to: number) => void>;

	// new expression with type args
	const a = new Set<(from: number, to: number) => void>();

	// other contextual keywords as type function params
	type C = (as: string, of: boolean) => void;

	// qualified type name
	type D = ns.from;

	// index signature
	type E = { [from: string]: number };

	// mapped type
	type F = { [from in 'a' | 'b']: number };

	// type predicate
	function fn1(from: unknown): from is string {
		return typeof from === 'string';
	}

	// asserts predicate
	function fn2(from: unknown): asserts from {
		if (!from) throw new Error();
	}
</script>
