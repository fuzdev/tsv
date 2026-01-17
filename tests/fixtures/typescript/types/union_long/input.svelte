<script lang="ts">
	// 100 chars - stays inline (at boundary)
	type Union100 = Aaaaaaaaaaaaaaaaaaaaa | Bbbbbbbbbbbbbbbbbbb | Ccccccccccccccccccc | Ddddddddddddd;

	// 101 chars - wraps (exceeds print_width)
	type Union101 =
		| Aaaaaaaaaaaaaaaaaaaaa
		| Bbbbbbbbbbbbbbbbbbb
		| Ccccccccccccccccccc
		| Dddddddddddddd;
</script>
