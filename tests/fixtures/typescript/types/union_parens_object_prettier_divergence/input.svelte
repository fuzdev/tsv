<script lang="ts">
	// Parenthesized union with multiline object members in intersection
	// Union breaks with indent after ( and before )
	let {
		a,
		b
	}: (
		| {
				a: string;
				b?: undefined;
			}
		| {
				a?: undefined;
				b: string;
			}
	) & {
		c: string;
	} = {};

	// Short parenthesized union stays flat
	type T = ({ a: string } | { b: string }) & C;
</script>
