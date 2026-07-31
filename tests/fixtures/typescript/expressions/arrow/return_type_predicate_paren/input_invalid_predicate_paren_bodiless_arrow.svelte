<script lang="ts">
	// `(B) => true` is a function type, so it is the predicate's whole type and
	// the arrow is left without a body.
	const f = (a: A): a is (B) => true;
</script>
