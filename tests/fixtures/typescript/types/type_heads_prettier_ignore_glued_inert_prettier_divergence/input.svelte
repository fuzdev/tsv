<script lang="ts">
	// alias `=` head: a glued directive is inert
	type A = /* prettier-ignore */ { x: 1 };

	// named-tuple element head
	type B = [label: /* prettier-ignore */ { x: 1 }, other: b];

	// mapped-type key, glued in-bracket
	type C = { [/* prettier-ignore */ K in keyof T]: V };

	// mapped-type value head
	type D = { [K in keyof T]: /* prettier-ignore */ { x: 1 } };
</script>
