<script>
	function* gen() {
		// contextual type keywords as identifiers
		yield string;
		yield number;
		yield boolean;
		yield any;
		yield never;
		yield unknown;
		yield object;
		yield symbol;
		yield bigint;

		// contextual module keywords as identifiers
		yield from;
		yield as;
		yield satisfies;
	}
</script>
