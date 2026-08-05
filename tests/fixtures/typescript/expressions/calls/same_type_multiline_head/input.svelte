<script lang="ts">
	// same-type args (two objects) with a source-multiline head: all args break out
	fn(
		{
			a: 1
		},
		{ b: 2 }
	);

	// flat same-type args stay inline
	fn({ a: 1 }, { b: 2 });
</script>
