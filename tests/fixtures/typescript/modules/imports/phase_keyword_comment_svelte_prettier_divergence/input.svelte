<script lang="ts">
	// block comment between import and the defer phase keyword
	import /* c1 */ defer * as ns1 from './a';

	// line comment between import and the defer phase keyword
	import // c2
		defer * as ns2 from './b';
</script>
