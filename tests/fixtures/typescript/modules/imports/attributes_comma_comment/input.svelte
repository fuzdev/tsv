<script>
	// before key
	import a from './a' with { /* c */ type: 'json' };

	// key → colon
	import b from './b' with { type /* c */: 'json' };

	// colon → value
	import c from './c' with { type: /* c */ 'json' };

	// value → close
	import d from './d' with { type: 'json' /* c */ };

	// between attributes (after the comma)
	import e from './e' with { type: 'json', /* c */ attr: 'value' };
</script>
