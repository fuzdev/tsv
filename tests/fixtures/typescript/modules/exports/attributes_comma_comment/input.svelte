<script>
	// before key
	export {a} from 'a' with {/* c */ type: 'json'};

	// key → colon
	export {b} from 'b' with {type /* c */: 'json'};

	// colon → value
	export * from 'c' with {type: /* c */ 'json'};

	// value → close
	export * from 'd' with {type: 'json' /* c */};

	// between attributes (after the comma)
	export {e} from 'e' with {type: 'json', /* c */ attr: 'value'};
</script>
