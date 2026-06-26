<script lang="ts">
	enum E {
		// A line comment before `=` stays after the name; `= value` indents one level.
		A = 1, // c
		// Two comments around the member stay distinct (prettier merges them — loss).
		B = 2 // c1 // c2
	}
</script>
