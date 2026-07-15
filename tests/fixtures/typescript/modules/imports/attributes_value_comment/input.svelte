<script>
	// a value-leading line comment breaks after the colon, indented one level
	import a from './a' with {
		type:
			// c
			'json'
	};

	// multiple value-leading line comments each stay on their own line
	import b from './b' with {
		type:
			// c1
			// c2
			'json'
	};

	// a block comment sharing the gap with a line comment also breaks onto its own line
	import c from './c' with {
		type:
			/* b */
			// c
			'json'
	};

	// the value comment on a non-first attribute breaks that attribute only
	import d from './d' with {
		type: 'json',
		resolution:
			// c
			'import'
	};
</script>
