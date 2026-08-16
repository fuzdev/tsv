<script lang="ts">
	// Block comment between the parameter name and its `:` stays where it was written
	type A = (x /* c */ : { a: number }) => void;

	// After the optional `?` marker, before the `:`
	type B = (x? /* c */ : { a: number }) => void;

	// A run of two blocks in the same gap, in order
	type C = (x /* c1 */ /* c2 */ : { a: number }) => void;

	// The name gap alongside the `:`→type gap
	type D = (x /* c1 */ : /* c2 */ { a: number }) => void;

	// The name gap alongside the trailing parameter gap
	type E = (x /* c1 */ : { a: number } /* c2 */) => void;

	// A constructor type hugs its single object parameter the same way
	type F = new (x /* c */ : { a: number }) => void;
</script>
