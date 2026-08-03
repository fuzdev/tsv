<script lang="ts">
	// A line comment before `=` stays after the name; `= type` indents one level.
	type A =
		// c
		number;

	// A trailing comment stays distinct from the gap comment in both formatters.
	type B =
		// c1
		string; // c2
</script>
