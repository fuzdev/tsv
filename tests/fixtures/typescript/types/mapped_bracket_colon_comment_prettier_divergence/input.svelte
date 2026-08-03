<script lang="ts">
	// Single comment: stays in its authored gap, before the value `:`.
	type A = {
		[K in keyof T] /* c */ : V;
	};

	// A run keeps order, each comment distinct.
	type B = {
		[K in keyof T] /* c1 */ /* c2 */ : V;
	};
</script>
