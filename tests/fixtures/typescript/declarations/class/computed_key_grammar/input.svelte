<script lang="ts">
	class C {
		// a computed member key is a single assignment expression, not a sequence or spread
		[a + b]() {}
		[(x, y)]() {}
		[a in b]() {}
		[c ? d : e] = 1;
		// an assignment key is parenthesized for clarity (like an object computed key)
		[(x = 0)] = 1;
		[(x = 0)]() {}
	}
</script>
