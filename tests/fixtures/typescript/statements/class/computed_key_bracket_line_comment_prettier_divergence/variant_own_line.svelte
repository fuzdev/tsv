<script lang="ts">
	class C {
		// A line comment before `=` stays after `]`; `= value` indents one level.
		[y] =
			// c
			2;

		// Two comments around the member stay distinct (prettier merges them — loss).
		[z] =
			// c1
			3; // c2
	}
</script>
