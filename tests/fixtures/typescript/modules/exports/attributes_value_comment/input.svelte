<script>
	// re-export named: a value-leading line comment breaks after the colon
	export { a } from './a' with {
		type:
			// c
			'json'
	};

	// re-export all: multiple value-leading line comments each stay on their own line
	export * from './b' with {
		type:
			// c1
			// c2
			'json'
	};
</script>
