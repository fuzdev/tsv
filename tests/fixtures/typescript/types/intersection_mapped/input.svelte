<script lang="ts">
	// Intersection with mapped type last - `& {` should hug
	type A = {[P in K]: T[P]} & {
		[P in Exclude<keyof T, K>]?: T[P];
	};

	// Simple type with mapped type
	type B = A & {
		[K in keyof U]: V;
	};

	// Multiple intersections - breaks after first &, hugs last `& {`
	type C = A &
		B & {
			[K in keyof U]: V;
		};
</script>
