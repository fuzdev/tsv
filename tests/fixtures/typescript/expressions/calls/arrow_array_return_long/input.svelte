<script lang="ts">
	// 100 visual chars - stays inline
	const a = arrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr.map(([aaaaaa, bbbbbb]) => [bbbbbb, aaaaaa]);

	// 101 visual chars - arrow params hug `map(`, array breaks
	const b = arrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr.map(([aaaaaa, bbbbbb]) => [
		bbbbbb,
		aaaaaa,
	]);
</script>
