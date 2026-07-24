<script lang="ts">
	// annotation head: a glued directive is inert
	let v: /* prettier-ignore */ {x:   1};

	// union child: same rule
	let u: /* prettier-ignore */ {x:   1} | c1;

	// property-signature host
	interface I {
		a: /* prettier-ignore */ {x:   1};
	}

	// return-type host
	function fn(): /* prettier-ignore */ {x:   1} {
		return { x: 1 };
	}

	// a multi-line value keeps its authored break; the parameter list breaks by
	// ordinary layout
	function fn2(a: /* prettier-ignore */ {
			x:   1;
		}, b: c1) {}
</script>
