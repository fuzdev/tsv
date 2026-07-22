<script lang="ts">
	// specifiers→from gap: block comment reflows inline
	import { a }

	/* c */
	from './a';

	// keyword→brace gap: block comment reflows inline
	import

	/* c */
	{ b } from './b';

	// default binding→from gap: block comment reflows inline
	import c

	/* c */
	from './c';
</script>
