<script lang="ts">
	type T = {
		// short intersection: stays inline
		[k: string]: A & B;
		// long intersection: hugs the colon line, members wrap with trailing &
		[k: string]: Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &
			Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &
			Ccccccccccccccccccccccccccccccccccccccccccc;
		// short union: stays inline
		[k: string]: A | B;
		// long union: breaks after colon to leading-pipe form
		[k: string]:
			| Aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
			| Bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
			| Ccccccccccccccccccccccccccccccccccccccccccc;
	};
</script>
