<script lang="ts">
	// Callback first, compact object last
	fn(
		(x) => {
			return x;
		},
		{ a: 1 }
	);

	// Callback first, array last
	fn(
		(x) => {
			return x;
		},
		[1, 2]
	);

	// Multiple args: string, callback, object
	fn(
		'str',
		(x) => {
			return x;
		},
		{ a: 1 }
	);
</script>
