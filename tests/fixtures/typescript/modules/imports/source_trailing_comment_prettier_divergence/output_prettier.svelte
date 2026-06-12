<script>
	// block comment between source and `;` stays after the source
	import {a} from './a' /* c */;

	// line comments between source and `;` stay where the user placed them
	import {b} from './b'; // 1
	// 2

	// block comment after import attributes stays after the `}`
	import c from './c' with {type: 'json' /* c */};
</script>
