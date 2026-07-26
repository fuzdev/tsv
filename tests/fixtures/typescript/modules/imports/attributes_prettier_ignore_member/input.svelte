<script lang="ts">
	// an own-line directive after the `{` freezes only the first attribute
	import a from './a' with {
		// prettier-ignore
		type:   'json',
		key: 'value'
	};

	// between two attributes
	import b from './b' with {
		type: 'json',
		// prettier-ignore
		key:   'value'
	};

	// an own-line block comment behaves identically
	import c from './c' with {
		/* prettier-ignore */
		type:   'json'
	};

	// a string key rides inside the frozen slice
	import d from './d' with {
		// prettier-ignore
		'k-k':   'value',
		type: 'json'
	};

	// a re-export host
	export { e } from './e' with {
		// prettier-ignore
		type:   'json',
		key: 'value'
	};

	// an `export *` host
	export * from './f' with {
		// prettier-ignore
		type:   'json'
	};
</script>
