<script lang="ts">
	// A block-body first callback keeps a "simple" tail arg inline after `}`
	// (isSimpleCallArgument). A template literal, unary, or update tail all qualify
	// as simple, so each stays inline rather than forcing all args to break.

	// template literal tail
	foo(() => {
		doThing();
	}, `a${x}b`);

	// unary tail
	foo(() => {
		doThing();
	}, -x);

	// update tail
	foo(() => {
		doThing();
	}, x++);
</script>
