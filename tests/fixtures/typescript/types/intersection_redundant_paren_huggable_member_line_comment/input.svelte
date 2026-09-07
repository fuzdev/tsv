<script lang="ts">
	// the same redundant-paren shell reaching the forced-multiline line-comment path: an
	// own-line comment at the last gap, so the earlier object-adjacent boundaries still hug

	// two huggable members, the comment at the last gap
	type First = {
		a: 1;
	} & {
		b: 2;
	} &
		// c1
		C;

	// the same run one boundary later, so the first gap is a non-object head
	type Head = B & {
		a: 1;
	} & {
		c: 2;
	} &
		// c2
		D;

	// a mapped type is huggable too
	type Mapped = {
		[K in T]: U;
	} & {
		a: 1;
	} &
		// c3
		C;

	// nested: an interface member
	interface Member {
		a: {
			b: 1;
		} & {
			c: 2;
		} &
			// c4
			D;
	}
</script>
