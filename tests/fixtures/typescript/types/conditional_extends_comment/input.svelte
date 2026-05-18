<script lang="ts">
	// Comment between check type and extends
	type A = T /* c */ extends string ? true : false;

	// Comment between extends and constraint
	type B = T extends /* c */ string ? true : false;

	// Multiple comments around extends
	type C = T /* c1 */ extends /* c2 */ string ? true : false;
</script>
