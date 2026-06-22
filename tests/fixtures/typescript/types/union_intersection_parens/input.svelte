<script lang="ts">
	// Intersection in union - parenthesized
	type T = A | (B & C);

	// Multiple intersections
	type U = (A & B) | (C & D);

	// Intersection at start
	type V = (A & B) | C;

	// Intersection in middle
	type W = A | (B & C) | D;

	// Intersection with object literal
	type X = A | (B & { c: C }) | D;

	// Already parenthesized (preserved)
	type Y = A | (B & C);
</script>
