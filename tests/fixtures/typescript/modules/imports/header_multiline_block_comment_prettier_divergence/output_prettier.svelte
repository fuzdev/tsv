<script lang="ts">
	// from→source gap: multiline block hangs the source, indented one level
	import {
		a /* x
y */
	} from './a';

	// specifiers→from gap: multiline block hangs the keyword
	import {
		b /* x
y */
	} from './b';

	// keyword→brace gap: multiline block hangs the specifiers
	import {
		/* x
y */
		c
	} from './c';
</script>
