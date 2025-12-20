<script lang="ts">
	// Union type: comment stays before, type indented
	interface B {
		b: // union
			X | Y;
	}

	// Intersection type: comment stays before, type NOT indented
	interface C {
		c: // intersection
		X & Y;
	}
</script>
