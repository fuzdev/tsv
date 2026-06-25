<script lang="ts">
	class C {
		// A line comment before `=` stays after the name; `= value` indents one level.
		a // c
		= 1;

		// Two comments around the construct stay distinct (prettier merges them — loss).
		b // c1
		= 2; // c2
	}
</script>
