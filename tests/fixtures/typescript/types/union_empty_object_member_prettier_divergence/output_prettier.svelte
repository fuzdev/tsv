<script lang="ts">
	// An interior comment in an empty union-member object is preserved with
	// bracket spacing; prettier tightens it.
	type Commented = A | {/* c */};
</script>
