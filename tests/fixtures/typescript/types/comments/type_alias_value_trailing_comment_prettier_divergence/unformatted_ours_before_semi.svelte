<script lang="ts">
	// block comment between value and ; stays before the ;
	type A = B /* c */;
	type U = A | B /* c */;
	type V = A & B /* c */;

	// line comment between value and ; moves after the ;
	type D = E // c
		;
	type W = A | B // c
		;
</script>
