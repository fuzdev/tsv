<script lang="ts">
	// 100 chars exactly - both stay inline (boundary before = break)
	type T = `prefix_${SomeVeryLongTypeNameCcccccccccccccccccccccccccccccccccccccccccccccccc}_suffix`;

	// 101 chars - both break at =, template line is 91 chars (under 100)
	type U =
		`prefix_${SomeVeryLongTypeNameCccccccccccccccccccccccccccccccccccccccccccccccccc}_suffix`;

	// After = break, template line is 100 chars exactly - both stay inline
	type V =
		`prefix_${SomeVeryLongTypeNameCccccccccccccccccccccccccccccccccccccccccccccccccccccccc}_suffix`;
</script>
