<script lang="ts">
	// A comment inside stripped grouping parens around a TAGGED TEMPLATE's tag, where the tagged
	// template is a chain's base. The run leads the chain from ahead of the whole chain doc, so
	// the chain keeps the layout the same document gets with the parens left out.

	(// c1
	a)`t`.g().h();

	(// c2
	a)()`t`.g().h();

	x = ((// c3
	a).b)`t`.g().h();

	f((
	/* c4 */
	a)`t`.g().h());

	function g() {
		return (// c5
		a)`t`.g().h();
	}

	h = () => (// c6
	a)`t`.g().h();
</script>
