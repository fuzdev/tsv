<script lang="ts">
	class C {
		// A line comment before `=` stays after the name; `= value` indents one level.
		a = 1; // c

		// Two comments around the construct stay distinct (prettier merges them — loss).
		b = 2; // c1 // c2
	}
</script>
