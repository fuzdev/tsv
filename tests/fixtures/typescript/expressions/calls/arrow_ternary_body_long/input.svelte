<script lang="ts">
	// Short - arrow inline
	a = b.map((x) => (x ? c : d));

	// 100 chars - at boundary, stays inline
	a = b.map((x) => (x ? ccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc : d));

	// 101 chars - exceeds, ternary breaks, params stay with map
	a = b.map((x) =>
		x ? cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc : d
	);

	// Long with comparison operator - ternary breaks, params stay with map
	aaaaaaaaaaaaaaaaaaaaaaaa = bbbbbbbbbbbbbbbbbbbbbb.map((x) =>
		x === cccccccccccccccccccccc ? ddddddddddddddddddddddddddddd! : x
	);

	// Very long params - params wrap too
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa =
		bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb.map((xxxxxxxxxxxxxxxxxxxxxxxxxxxx) =>
			xxxxxxxxxxxxxxxxxxxxxxxxxxxx === yyyyyyyyyyyyyyyyyyyyyyyyyyyyyy
				? zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz!
				: xxxxxxxxxxxxxxxxxxxxxxxxxxxx
		);
</script>
