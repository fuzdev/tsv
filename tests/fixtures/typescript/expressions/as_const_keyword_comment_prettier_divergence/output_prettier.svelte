<script lang="ts">
	// Block comment between `as` and `const` stays put (cf. `as <type>`)
	1 /* c */ as const;

	// Line comment after `as` keeps `const` on the next line
	1 as const; // c
</script>
