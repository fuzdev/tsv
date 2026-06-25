<script lang="ts">
	enum E {
		// A line comment before `=` stays after the name; `= value` indents one level.
		A // c
		= 1,
		// Two comments around the member stay distinct (prettier merges them — loss).
		B // c1
		= 2 // c2
	}
</script>
