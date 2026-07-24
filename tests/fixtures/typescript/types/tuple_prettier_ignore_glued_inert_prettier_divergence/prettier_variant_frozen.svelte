<script lang="ts">
	// a glued directive is inert — the inline tuple formats normally
	type A = [a, /* prettier-ignore */ {x:   1}, b];

	// other comments in the run don't change the classification
	type B = [a, /* prettier-ignore */ /* other */ {x:   1}, b];

	// a multi-line member keeps its authored break; the tuple breaks by ordinary layout
	type C = [a, /* prettier-ignore */ {
			x:   1
		}, b];
</script>
