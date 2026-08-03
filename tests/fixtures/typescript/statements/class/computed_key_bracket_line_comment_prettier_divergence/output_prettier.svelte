<script lang="ts">
	class C {
		// A line comment before `=` stays after `]`; `= value` indents one level.
		[y] = 2; // c

		// Two comments around the member stay distinct (prettier merges them — loss).
		[z] = 3; // c1 // c2
	}
</script>
