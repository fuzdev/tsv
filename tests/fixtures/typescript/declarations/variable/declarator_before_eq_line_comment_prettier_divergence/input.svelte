<script lang="ts">
	// A line comment before `=` stays after the name; `= value` indents one level.
	const a // c
		= 1;

	// Two comments around the construct stay distinct (prettier merges them — loss).
	const b // c1
		= 2; // c2
</script>
