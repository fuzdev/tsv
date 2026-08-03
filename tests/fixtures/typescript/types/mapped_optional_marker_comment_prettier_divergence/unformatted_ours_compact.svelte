<script lang="ts">
	// Comment before the optional marker stays before it.
	type A = {
		[K in keyof T]/* c1 */?:V;
	};

	// Comment between the marker and the value `:` trails the marker,
	// for every marker spelling.
	type B = {
		[K in keyof T]?/* c2 */:V;
	};
	type C = {
		[K in keyof T]+?/* c3 */:V;
	};
	type D = {
		[K in keyof T]-?/* c4 */:V;
	};
</script>
