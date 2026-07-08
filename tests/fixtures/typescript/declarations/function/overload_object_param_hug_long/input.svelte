<script lang="ts">
	// A bodyless function declaration (overload / ambient signature) with a single
	// object-type parameter hugs it: `(o: {` stays on the signature line and the
	// object type breaks — matching the function-with-body, method, and arrow forms.

	// 100 chars - stays inline
	function fn(o: { a: string; b: string; c: string; d: string; padding_________: string }): boolean;

	// 101 chars - hugs (the object type breaks; the param list does not)
	function fn(o: {
		a: string;
		b: string;
		c: string;
		d: string;
		padding__________: string;
	}): boolean;
</script>
