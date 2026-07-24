<script lang="ts">
	// a glued block directive freezes the annotation's type
	let v: /* prettier-ignore */ {x:   1};

	// glued before a union: the whole union freezes
	let u: /* prettier-ignore */ {x:   1} | c1;

	// property-signature host
	interface I {
		a: /* prettier-ignore */ {x:   1};
	}

	// return-type host
	function fn(): /* prettier-ignore */ {x:   1} {
		return { x: 1 };
	}
</script>
