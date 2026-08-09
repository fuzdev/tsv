<script lang="ts">
	// A block hugging the next element leads it; the line comment forces multiline
	type A = [
		string, // t
		number, /* c */ boolean
	];

	// Before-comma and after-comma blocks in one gap each keep their side
	type B = [
		string, // t
		number /* c1 */, /* c2 */ boolean
	];
</script>
