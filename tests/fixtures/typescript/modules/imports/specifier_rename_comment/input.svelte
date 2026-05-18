<script lang="ts">
	// block comment between imported name and as keyword
	import {a /* c */ as b} from 'x';

	// type specifier with block comment
	import {type c /* c */ as d} from 'x';

	// export specifier rename comment
	export {a /* c */ as b};

	// export specifier rename comment with re-export
	export {a /* c */ as b} from 'y';
</script>
