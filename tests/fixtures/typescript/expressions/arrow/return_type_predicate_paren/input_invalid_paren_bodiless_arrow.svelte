<script lang="ts">
	// Without a type predicate the shape is invalid the same way: `(B) => true`
	// is read as a function type, so the arrow is left without a body.
	const f = (a: A): (B) => true;
</script>
