<script lang="ts">
	// At 100 columns the authored pair and the list stay on one line
	fn(a < b, c > (d, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee));
	// At 101 columns the list breaks, one element a line, and the pair stays
	fn(a < b, c > (d, eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee));
	// A paired comparison too wide for a line breaks inside its pair
	fn(
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa <
			bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
		d > (e, f)
	);
</script>
