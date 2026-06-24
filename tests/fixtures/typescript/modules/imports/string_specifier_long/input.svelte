<script>
	// Fits at exactly 100 — the string-specifier list stays inline.
	import { 'aaaaaaaaaa' as bbbbbbbb01, 'cccccccccc' as dddddddd01 } from './eeeeeeeeeeeeeeeeeeeeee';
	// At 101 — the string-specifier list breaks one per line.
	import {
		'aaaaaaaaaa' as bbbbbbbb02,
		'cccccccccc' as dddddddd02
	} from './eeeeeeeeeeeeeeeeeeeeeee';
</script>
