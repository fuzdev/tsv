<script lang="ts">
	// a blank line between params is preserved (forces expansion), matching prettier

	// function type
	type F = (
		a: number,

		b: number
	) => void;

	// constructor type
	type C = new (
		a: number,

		b: number
	) => void;

	// method signature
	interface I {
		method(
			a: number,

			b: number
		): void;
	}

	// call signature
	type Call = {
		(
			a: number,

			b: number
		): void;
	};

	// no blank line stays inline
	type G = (a: number, b: number) => void;
</script>
