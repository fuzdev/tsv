<script lang="ts">
	// Short - fits on one line
	x = a.map((y) => y * 2);

	// 100 chars - at boundary (arrow stays on same line)
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = bbbbbbbbbbbbbbb.map((x) => x * 2222222222222222222222222222);

	// 101 chars - exceeds (call stays with =, arrow params wrap)
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = bbbbbbbbbbbbbbbb.map(
		(x) => x * 2222222222222222222222222222,
	);
</script>
