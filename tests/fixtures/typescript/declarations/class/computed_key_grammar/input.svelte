<script lang="ts">
	class C {
		// a computed member key is a single assignment expression, not a sequence or spread
		[a + b]() {}
		[(x, y)]() {}
		[a in b]() {}
		[c ? d : e] = 1;
	}
</script>
