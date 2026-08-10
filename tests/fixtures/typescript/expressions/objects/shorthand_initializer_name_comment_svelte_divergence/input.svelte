<script lang="ts">
	// a comment between a shorthand property's name and its initializer keeps its place: the
	// property's own doc prints the whole `a1 = 1`, so the list seam must not claim inside it
	({ a1 /* c1 */ = 1, b1 });

	// the same binding on the last property, where the closer ends the run instead of a comma
	({ b2, a2 /* c2 */ = 1 });

	// past the initializer the comment trails the property, as in every other list family
	({ a3 = 1 /* c3 */, b3 });
</script>
