<script lang="ts">
	// A typeless index signature (no `: ValueType`) is grammatically valid; the
	// missing value type is a tsc checker error (TS1021), not a parse error.
	type A = { [a: string] };

	// The key type is still parsed and can be any type (its validity — e.g. it must
	// be string/number/symbol — is a deferred tsc checker error, not a parse error).
	type B = { [a: boolean] };

	// Mixed with a typed member (a separator between members is still required)
	type C = { [a: string]; b: number };

	// In an interface body
	interface I {
		[a: string];
	}

	// With a readonly modifier
	interface J {
		readonly [a: string];
	}

	// Followed by another member across a line break (ASI separates them)
	interface K {
		[a: string];
		b: number;
	}

	// In a function parameter and return type annotation
	function fn(o: { [k: string] }): { [k: string] } {
		return o;
	}
</script>
