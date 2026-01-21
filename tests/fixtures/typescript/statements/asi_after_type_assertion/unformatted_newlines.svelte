<script lang="ts">
	// Bracket after type - ASI should trigger (new statement)
	const a = x as A
	[B];

	// Plus after type - no ASI (binary expression)
	const b = x as A
	+ z;

	// Minus after type - no ASI (binary expression)
	const c = x as A
	- z;
</script>
