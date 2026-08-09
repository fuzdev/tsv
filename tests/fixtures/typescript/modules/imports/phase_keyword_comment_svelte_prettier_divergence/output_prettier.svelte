<script lang="ts">
	// block comment between import and the defer phase keyword
	import defer /* c1 */ * as ns1 from './a';

	// line comment between import and the defer phase keyword
	import defer // c2
	* as ns2 from './b';
</script>
