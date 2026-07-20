<script lang="ts">
	// A `//` comment authored on the `=` line stays there (prettier relocates it).
	type T =
		// c
		Ref;

	// A second case with a breaking union RHS: the comment still stays on `=`.
	type U =
		// note
		Aaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbb | Ccccccccccccccccccc | Ddddddddddddddddddd;
</script>
