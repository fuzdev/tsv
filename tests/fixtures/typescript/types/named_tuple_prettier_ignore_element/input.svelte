<script lang="ts">
	// a glued block directive freezes the element type it precedes
	type A = [label: /* prettier-ignore */ {x:   1}, other: b];

	// own-line before a union element: Rule A — the first member freezes
	type B = [
		label:
			// prettier-ignore
			{x:   1} | c1,
		other: b
	];
</script>
