<script lang="ts">
	// A rest parameter carries a `?`. Plain rest (`...b`) is unaffected.

	// value-position binding param
	const arrow = (...a) => 1;

	// call signature
	interface Call {
		(...a: unknown[]): void;
	}

	// construct signature
	interface Ctor {
		new (...a: unknown[]): void;
	}

	// function type
	type Fn = (...a: unknown[]) => void;

	// plain rest — no divergence
	const plain = (...b) => 1;
</script>
