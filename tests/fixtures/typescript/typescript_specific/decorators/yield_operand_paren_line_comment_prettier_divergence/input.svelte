<script lang="ts">
	// A `//` comment in the grouping `(`→operand gap of a `yield` whose operand is a decorated
	// class: the hanging pair the comment needs is the class's pair too, so it takes no second
	function* gen() {
		yield ( // c
			@dec
			class {}
		);
		yield (
			// c
			@dec
			class {}
		);
	}
</script>
