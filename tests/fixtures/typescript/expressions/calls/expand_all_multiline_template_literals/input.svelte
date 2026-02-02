<script lang="ts">
	// Two multiline template literals - Prettier expands all args
	fn(
		`
a
b
`,
		`
c
d
`,
	);
</script>
